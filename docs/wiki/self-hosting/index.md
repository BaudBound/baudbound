---
title: Self-Hosting
description: Self-host the BaudBound editor and schema service.
tags: [self-hosting, operations]
---
# Self-Hosting

This section is for operators who want to run the browser editor or public JSON Schemas on their own server. You do not need to self-host either service to use BaudBound: the public editor is available at [editor.baudbound.app](https://editor.baudbound.app/), and runner package validation works offline.

## Before you begin

You need:

- A server with Docker Engine and the Docker Compose plugin.
- Permission to create DNS records for the public hostname.
- An HTTPS reverse proxy such as Nginx, Caddy, or Traefik.
- `curl` for local health checks.
- Access to container logs and firewall settings on the server.

The examples bind containers to `127.0.0.1`, so they are reachable only by software on the same server. The reverse proxy is responsible for public HTTPS access. Do not expose the container ports directly to the internet.

## Self-host the editor

The editor is stateless: it does not require a database or persistent container volume. Projects remain in each user's browser and exported `.bbs` packages are downloaded to that user's machine. Removing or replacing the editor container does not delete projects stored in users' browsers.

### Docker Compose

Create a dedicated directory, then open its Compose file in the system editor:

```text
sudo mkdir -p /opt/baudbound-editor
sudoedit /opt/baudbound-editor/compose.yaml
```

Paste the following content into the file opened by `sudoedit`, save it, and close the editor:

```yaml
services:
  editor:
    image: ghcr.io/natroutter/baudbound-editor:latest
    container_name: baudbound-editor
    restart: unless-stopped
    ports:
      - "127.0.0.1:3000:3000"
    environment:
      NODE_ENV: production
      EDITOR_URL: "https://editor.example.com"
```

Replace `editor.example.com` with the public HTTPS hostname. After saving the file, enter the directory and start the container:

```text
cd /opt/baudbound-editor
docker compose pull
docker compose up -d
docker compose ps
```

`docker compose ps` should show `baudbound-editor` as running. Before configuring the reverse proxy, confirm that the server itself can reach the editor:

```text
curl --fail http://127.0.0.1:3000/
```

If this command fails, inspect the container before continuing:

```text
docker compose logs editor
```

`EDITOR_URL` controls canonical and social metadata in the published container. Pin a release or `sha-*` image tag instead of `latest` when updates must be promoted manually.

After the local check succeeds, continue to [Reverse proxy](#reverse-proxy). If you also want to host the public schemas, configure that container first so both hostnames can be added to the proxy together.

## Schema host

The schema host is optional. It serves public JSON Schema documents for external tools and documentation. The runner does not contact it while validating packages.

Create a separate directory so editor and schema containers can be updated independently, then open its Compose file:

```text
sudo mkdir -p /opt/baudbound-schemas
sudoedit /opt/baudbound-schemas/compose.yaml
```

Paste the following content into the file opened by `sudoedit`, save it, and close the editor:

```yaml
services:
  schemas:
    image: ghcr.io/natroutter/baudbound-schemas:latest
    container_name: baudbound-schemas
    restart: unless-stopped
    ports:
      - "127.0.0.1:8085:80"
```

After saving the file, enter the directory, start the container, and test its health endpoint:

```text
cd /opt/baudbound-schemas
docker compose pull
docker compose up -d
curl --fail http://127.0.0.1:8085/healthz
```

The final command should print `ok`. Point the schema hostname's HTTPS reverse proxy to `http://127.0.0.1:8085`, then verify the public `/healthz` URL.

Schema documents are served as `application/schema+json` with `X-Content-Type-Options: nosniff`. The container uses a five-minute revalidation cache so clients can cache briefly without hiding schema updates indefinitely. Unknown paths return `404` and directory listing is disabled.

## Reverse proxy {.tabset}

Complete one tab only. Every example uses these placeholder hostnames:

- `editor.example.com` for the editor at port `3000`.
- `schemas.example.com` for the schema host at port `8085` on the server or port `80` inside its container.

Replace both names with real DNS names. Create their DNS records before requesting certificates. Make sure inbound TCP ports `80` and `443` reach the reverse proxy. Keep the Compose port bindings on `127.0.0.1`. They are useful for local health checks and should not be exposed publicly.

If you are hosting only one service, configure only its hostname. Do not point both hostnames to the same upstream port.
{.is-info}

### Nginx

These instructions assume Nginx runs directly on the server. Create a site configuration using the location used by your distribution, such as `/etc/nginx/conf.d/baudbound.conf` or `/etc/nginx/sites-available/baudbound.conf`.

Start with HTTP virtual hosts so Nginx can load the configuration before certificates exist:

```nginx
map $http_upgrade $connection_upgrade {
    default upgrade;
    '' close;
}

server {
    listen 80;
    listen [::]:80;
    server_name editor.example.com;

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection $connection_upgrade;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}

server {
    listen 80;
    listen [::]:80;
    server_name schemas.example.com;

    location / {
        proxy_pass http://127.0.0.1:8085;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

Replace both hostnames and remove the server block for a service you are not hosting. The `map` block preserves WebSocket upgrades without forcing every request to use an upgrade connection.

On Debian and Ubuntu, enable a file created under `sites-available` by linking it into `sites-enabled`:

```text
sudo ln -s /etc/nginx/sites-available/baudbound.conf /etc/nginx/sites-enabled/baudbound.conf
```

Do not run that command when the configuration is already loaded from `conf.d` or already linked. Test before reloading:

```text
sudo nginx -t
sudo systemctl reload nginx
```

The test must succeed. Confirm both HTTP hostnames reach their intended services before requesting certificates.

Install Certbot and its Nginx integration by selecting the server's Linux distribution in the official [Certbot Nginx instructions](https://certbot.eff.org/instructions?ws=nginx). Then request certificates and configure HTTP-to-HTTPS redirects:

```text
sudo certbot --nginx --redirect -d editor.example.com -d schemas.example.com
```

Remove the `-d` argument for a service you are not hosting. Certbot should report that it installed the certificate successfully. Test Nginx and certificate renewal:

```text
sudo nginx -t
sudo certbot renew --dry-run
```

Both commands must succeed. Nginx documents `proxy_pass`, forwarded headers, and WebSocket handling in its official [`ngx_http_proxy_module` reference](https://nginx.org/en/docs/http/ngx_http_proxy_module.html).

### Nginx Proxy Manager

Nginx Proxy Manager commonly runs in Docker. Its container cannot reach the server's `127.0.0.1` ports directly. Put Nginx Proxy Manager and the BaudBound containers on one private Docker network instead.

Create the shared network once:

```text
docker network create proxy
```

Add the external network to the Nginx Proxy Manager Compose file and attach its application service to it:

```yaml
services:
  app:
    networks:
      - proxy

networks:
  proxy:
    external: true
```

The service may have a different name in your existing Compose file. Add `networks: [proxy]` to the service that runs Nginx Proxy Manager. Do not create a second `app` service.

Add the same network to `/opt/baudbound-editor/compose.yaml`:

```yaml
services:
  editor:
    networks:
      - proxy

networks:
  proxy:
    external: true
```

If the schema host is installed, add it to `/opt/baudbound-schemas/compose.yaml`:

```yaml
services:
  schemas:
    networks:
      - proxy

networks:
  proxy:
    external: true
```

Recreate all edited Compose projects so Docker attaches the existing containers to the network:

```text
docker compose up -d
```

Run that command once from each edited Compose directory.

In Nginx Proxy Manager, open **Hosts > Proxy Hosts** and create the editor host:

| Field | Value |
| --- | --- |
| Domain Names | `editor.example.com` |
| Scheme | `http` |
| Forward Hostname / IP | `baudbound-editor` |
| Forward Port | `3000` |
| Websockets Support | Enabled |

Open the **SSL** tab, request a new Let's Encrypt certificate, enable **Force SSL**, accept the Let's Encrypt terms, and save the host.

Create a second proxy host for schemas when it is installed:

| Field | Value |
| --- | --- |
| Domain Names | `schemas.example.com` |
| Scheme | `http` |
| Forward Hostname / IP | `baudbound-schemas` |
| Forward Port | `80` |

Request its certificate and enable **Force SSL** as well. Leave the **Advanced** field empty unless another documented requirement applies. The project describes Proxy Hosts and Let's Encrypt support in the official [Nginx Proxy Manager guide](https://nginxproxymanager.com/guide/).

### Caddy

These instructions assume Caddy runs directly on the server. Open `/etc/caddy/Caddyfile` and add:

```caddyfile
editor.example.com {
    reverse_proxy 127.0.0.1:3000
}

schemas.example.com {
    reverse_proxy 127.0.0.1:8085
}
```

Remove the block for a service you are not hosting. Caddy obtains and renews public TLS certificates automatically when DNS is correct and ports `80` and `443` reach it. Its reverse proxy also supplies the standard forwarded headers without manual header rules.

Validate and reload the configuration:

```text
sudo caddy validate --config /etc/caddy/Caddyfile
sudo systemctl reload caddy
```

The validation must succeed before reloading. See Caddy's official [`reverse_proxy` documentation](https://caddyserver.com/docs/caddyfile/directives/reverse_proxy) for its upstream, header, and WebSocket behavior.

If Caddy itself runs in Docker, do not use these loopback upstreams. Attach Caddy and the BaudBound containers to a shared Docker network as described in the Nginx Proxy Manager tab, then use `baudbound-editor:3000` and `baudbound-schemas:80` as the upstreams.

### Traefik

This example uses Traefik's Docker provider. It assumes Traefik already has:

- An HTTPS entrypoint named `websecure`.
- An ACME certificate resolver named `letsencrypt`.
- The Docker provider with containers hidden by default.
- Access to a shared external Docker network named `proxy`.

If your entrypoint, resolver, or network has a different name, update every matching label below. Create the network once when it does not already exist:

```text
docker network create proxy
```

Attach the Traefik service itself to that network. Then update `/opt/baudbound-editor/compose.yaml` with the network and labels:

```yaml
services:
  editor:
    networks:
      - proxy
    labels:
      - "traefik.enable=true"
      - "traefik.docker.network=proxy"
      - "traefik.http.routers.baudbound-editor.rule=Host(`editor.example.com`)"
      - "traefik.http.routers.baudbound-editor.entrypoints=websecure"
      - "traefik.http.routers.baudbound-editor.tls=true"
      - "traefik.http.routers.baudbound-editor.tls.certresolver=letsencrypt"
      - "traefik.http.services.baudbound-editor.loadbalancer.server.port=3000"

networks:
  proxy:
    external: true
```

When the schema host is installed, update `/opt/baudbound-schemas/compose.yaml`:

```yaml
services:
  schemas:
    networks:
      - proxy
    labels:
      - "traefik.enable=true"
      - "traefik.docker.network=proxy"
      - "traefik.http.routers.baudbound-schemas.rule=Host(`schemas.example.com`)"
      - "traefik.http.routers.baudbound-schemas.entrypoints=websecure"
      - "traefik.http.routers.baudbound-schemas.tls=true"
      - "traefik.http.routers.baudbound-schemas.tls.certresolver=letsencrypt"
      - "traefik.http.services.baudbound-schemas.loadbalancer.server.port=80"

networks:
  proxy:
    external: true
```

Recreate both edited projects:

```text
cd /opt/baudbound-editor
docker compose up -d
cd /opt/baudbound-schemas
docker compose up -d
```

Skip the schema commands when that service is not installed. Inspect Traefik's logs and dashboard to confirm that both routers have certificates and healthy services. Traefik's official Docker documentation explains that labels define routing and that the load-balancer port label selects the container port: [Docker provider](https://doc.traefik.io/traefik/reference/install-configuration/providers/docker/) and [Docker routing](https://doc.traefik.io/traefik/reference/routing-configuration/other-providers/docker/).

After completing any proxy tab, verify both the local upstream and public HTTPS URL:

```text
curl --fail http://127.0.0.1:3000/
curl --fail https://editor.example.com/
curl --fail http://127.0.0.1:8085/healthz
curl --fail https://schemas.example.com/healthz
```

Run only the checks for installed services. Open the public editor in a browser, create a small test project, and export it. The schema health endpoint should print `ok`. Do not indefinitely cache editor HTML. Hashed static assets can use their normal immutable cache headers.

## Operate, update, and roll back

Run commands from the service's `/opt/baudbound-*` directory:

```text
docker compose ps
docker compose logs --tail 100
docker compose restart
```

For controlled production environments, replace `latest` with a reviewed release tag or immutable digest. To update:

1. Save the currently deployed image reference.
2. Change the Compose image to the new reviewed tag.
3. Run `docker compose pull` and `docker compose up -d`.
4. Test the local health/page and public HTTPS URL.
5. Inspect `docker compose logs --tail 100`.

If verification fails, restore the previous image reference and run `docker compose up -d` again. The editor container is stateless, but users should export important browser-local projects before browser cleanup or workstation migration.

## Remove a self-hosted service

Removal stops the selected container and removes its Compose-created network. Run it only from the matching service directory:

```text
docker compose down
```

After confirming the container is gone, remove the reverse proxy host and DNS record. Deleting `/opt/baudbound-editor` or `/opt/baudbound-schemas` removes local Compose configuration. It does not delete editor projects stored in users' browsers. Do not use `docker compose down -v` as a habit because future deployments may add persistent volumes.

## Public verification checklist

- The public URL uses a valid HTTPS certificate and redirects HTTP to HTTPS.
- The editor loads, creates a project, and exports a test package.
- The schema `/healthz` endpoint returns `ok` and a known schema URL returns JSON.
- Container ports remain bound to `127.0.0.1`, not directly to a public interface.
- The reverse proxy preserves `Host` and forwarded protocol information.
- HTML is not cached indefinitely, and schema cache headers match the intended update policy.
- Logs contain no private package data or proxy authorization credentials.

Running the runner continuously is separate from self-hosting these web services. See [Linux Background Service](../runner/linux-background-service.md).

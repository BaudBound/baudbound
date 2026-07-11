---
title: Self-Hosting
description: Self-host the BaudBound editor and schema service.
tags: [self-hosting, operations]
---
# Self-Hosting

This section is for operators who want to run the browser editor or public JSON Schemas on their own server. You do not need to self-host either service to use BaudBound: the public editor is available at [editor.baudbound.app](https://editor.baudbound.app/), and runner package validation works offline.

## Before you begin

You need:

- a server with Docker Engine and the Docker Compose plugin;
- permission to create DNS records for the public hostname;
- an HTTPS reverse proxy such as Nginx, Caddy, or Traefik; and
- `curl` for local health checks; and
- enough familiarity with the server to inspect container logs and firewall rules.

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

### Reverse proxy

Create a DNS record that points the chosen hostname to the server. Configure the HTTPS virtual host to proxy requests to `http://127.0.0.1:3000`. A minimal Nginx location block is:

```nginx
location / {
    proxy_pass http://127.0.0.1:3000;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}
```

This block belongs inside the Nginx `server` block for the HTTPS hostname; it is not a complete Nginx configuration. Configure a valid TLS certificate before sharing the URL. Do not indefinitely cache HTML responses; hashed Next.js assets can use normal immutable caching.

Open the public HTTPS URL in a browser and create a small test project. Once that works, update the editor from `/opt/baudbound-editor` with:

```text
docker compose pull
docker compose up -d
```

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

To run scripts continuously on a headless Linux host, follow the dedicated [Linux Background Service](linux-background-service.md) guide.

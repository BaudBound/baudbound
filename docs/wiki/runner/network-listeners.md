---
title: Webhooks, WebSockets, and Network Access
description: Configure and test BaudBound network listeners safely before exposing them beyond one machine.
tags: [runner, webhooks, websockets, networking, security]
---
# Webhooks, WebSockets, and Network Access

BaudBound can accept HTTP webhook requests and WebSocket messages while a background runner is active. These listeners are local infrastructure components, not a hosted API gateway. The operator owns firewall, TLS, authentication, rate limiting, and reverse-proxy policy.

## Network basics

| Term | Meaning |
| --- | --- |
| **Bind address** | Local interface on which the runner accepts connections |
| **Port** | TCP port owned by the listener process |
| `127.0.0.1` | IPv4 loopback. Reachable only from the same machine. |
| `0.0.0.0` | Every IPv4 interface. Potentially reachable from a LAN or wider network. |
| **Reverse proxy** | Separate server that receives client traffic and forwards approved requests to the runner |

Defaults are intentionally local:

```toml
[webhooks]
bind = "127.0.0.1"
port = 43891
max_body_bytes = 1048576

[websockets]
bind = "127.0.0.1"
port = 43892
max_message_bytes = 1048576
max_connections = 128
```

The corresponding trigger-family switches under `[triggers]` must also be enabled. Configuration changes require the running service to restart.

> Changing a bind address to `0.0.0.0` can expose every registered route to other machines. BaudBound does not automatically add TLS, user authentication, firewall rules, or Internet-safe request policy.
{.is-warning}

## Webhook routes

A Webhook trigger constructs its route from **Hook name**:

```text
/events/HOOK_NAME
```

A node with method `POST` and hook name `deploy` matches `POST /events/deploy`. The configured method must match exactly. Query parameters do not change route selection.

The trigger output includes:

| Output | Meaning |
| --- | --- |
| `method` | Request method |
| `path` | Matched path without query text |
| `headers` | Parsed request headers |
| `query` | Parsed query-string object |
| `body` | Raw request body |
| `json` | Parsed JSON object when the body is valid JSON |
| `response` | Immediate or waiting response state |

Requests larger than `max_body_bytes` are rejected before workflow execution. Invalid JSON does not remove the raw `body`. It leaves parsed JSON unavailable or empty according to runtime handling.

### Immediate response

When **Wait for response node** is disabled, the listener sends the configured fallback status, content type, and body without waiting for downstream actions. The workflow still runs from the request.

### Workflow-owned response

When waiting is enabled, a reachable **Webhook Response** node must send one response for that request. It can set status, content type, headers, and a variable-resolved body.

If no response arrives before **Response timeout seconds**, the listener uses the trigger's fallback response. A response node reached without a waiting request, or reached twice for one request, fails rather than sending an unrelated response.

## Test a webhook locally

1. Enable webhooks in Config.
2. Keep bind `127.0.0.1` and port `43891`.
3. Import, approve, and enable a script with a POST webhook named `tutorial`.
4. Start or reload the background runner.
5. Send a request from the same machine.

### Client command {.tabset}

#### PowerShell

```powershell
Invoke-WebRequest `
  -Method Post `
  -Uri "http://127.0.0.1:43891/events/tutorial" `
  -ContentType "application/json" `
  -Body '{ "message": "hello" }'
```

#### Linux shell

```bash
curl --fail-with-body \
  --request POST \
  --header 'Content-Type: application/json' \
  --data '{ "message": "hello" }' \
  http://127.0.0.1:43891/events/tutorial
```

Inspect the HTTP status and body, then confirm the related run ID in Runs.

## WebSocket routes and messages

A WebSocket trigger uses its configured **Path** exactly. The path must begin with `/`, for example `/events/messages`. A client connects to:

```text
ws://127.0.0.1:43892/events/messages
```

Each accepted connection receives a runner-generated `connection_id`. Every inbound text message starts a run with outputs including the connection ID, route path, message text, and connection metadata.

**WebSocket Write** must use the connection ID from that trigger run. The runner keeps a connection registry and refuses unknown or disconnected IDs. This prevents a workflow from treating an arbitrary string as another client connection.

`max_message_bytes` rejects oversized messages. `max_connections` limits simultaneous accepted connections. Reload removes routes that are no longer registered and closes connections that cannot remain associated with an active route.

## Test WebSockets locally

Use a WebSocket client you trust. One cross-platform option is `wscat`, which requires Node.js and is not part of BaudBound:

```text
npx wscat --connect ws://127.0.0.1:43892/events/messages
```

Type a text message and press Enter. Confirm that one run appears with the same message and a connection ID. If the workflow contains WebSocket Write, its reply appears in the client.

Do not install a test dependency on a production server solely to prove the listener. A controlled workstation can test through the intended proxy path instead.

## Reverse proxy {.tabset}

Use two public hostnames so webhook and WebSocket paths cannot collide:

- `hooks.example.com` forwards to `127.0.0.1:43891`.
- `socket.example.com` forwards to `127.0.0.1:43892`.

Replace both names with real DNS names. Complete one proxy tab. TLS does not authenticate callers, so add the access control, source restrictions, and rate limits required by your deployment.

### Nginx

This example assumes Nginx runs directly on the runner machine. Add each block to its secured HTTPS virtual host.

Webhook host:

```nginx
location / {
    proxy_pass http://127.0.0.1:43891;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
    client_max_body_size 1m;
}
```

WebSocket host:

```nginx
location / {
    proxy_pass http://127.0.0.1:43892;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}
```

Test and reload Nginx:

```text
sudo nginx -t
sudo systemctl reload nginx
```

The test must succeed. See Nginx's official [`ngx_http_proxy_module` reference](https://nginx.org/en/docs/http/ngx_http_proxy_module.html) for proxy and WebSocket behavior.

### Nginx Proxy Manager

Nginx Proxy Manager normally runs in Docker. A container cannot reach the host runner through `127.0.0.1`.

Find the Docker host gateway:

```text
docker network inspect bridge --format '{{(index .IPAM.Config 0).Gateway}}'
```

The command should print one private IP address, commonly `172.17.0.1`. Set both runner bind addresses to that exact value in `config.toml`, then restart BaudBound. Do not use `0.0.0.0` for this setup.

Add this mapping to the Nginx Proxy Manager application service in its Compose file:

```yaml
extra_hosts:
  - "host.docker.internal:host-gateway"
```

Recreate Nginx Proxy Manager with `docker compose up -d`. Create two entries under **Hosts > Proxy Hosts**.

Webhook host:

| Field | Value |
| --- | --- |
| Domain Names | `hooks.example.com` |
| Scheme | `http` |
| Forward Hostname / IP | `host.docker.internal` |
| Forward Port | `43891` |

WebSocket host:

| Field | Value |
| --- | --- |
| Domain Names | `socket.example.com` |
| Scheme | `http` |
| Forward Hostname / IP | `host.docker.internal` |
| Forward Port | `43892` |
| Websockets Support | Enabled |

For both hosts, request a certificate in the **SSL** tab and enable **Force SSL**. The Docker mapping is documented in [Docker Compose networking](https://docs.docker.com/compose/how-tos/networking/). Proxy Host and certificate setup are documented in the [Nginx Proxy Manager guide](https://nginxproxymanager.com/guide/).

### Caddy

This example assumes Caddy runs directly on the runner machine. Add both sites to `/etc/caddy/Caddyfile`:

```caddyfile
hooks.example.com {
    reverse_proxy 127.0.0.1:43891
}

socket.example.com {
    reverse_proxy 127.0.0.1:43892
}
```

Caddy handles WebSocket upgrades and standard forwarded headers automatically. Validate and reload the configuration:

```text
sudo caddy validate --config /etc/caddy/Caddyfile
sudo systemctl reload caddy
```

The validation must succeed. See Caddy's official [`reverse_proxy` documentation](https://caddyserver.com/docs/caddyfile/directives/reverse_proxy).

If Caddy runs in Docker, use the Docker host gateway preparation from the Nginx Proxy Manager tab. Replace both `127.0.0.1` upstreams with `host.docker.internal`.

### Traefik

This example uses Traefik's file provider. It assumes an HTTPS entrypoint named `websecure` and an ACME certificate resolver named `letsencrypt`.

When Traefik runs in Docker, add this mapping to its Compose service and recreate the container:

```yaml
extra_hosts:
  - "host.docker.internal:host-gateway"
```

Follow the Nginx Proxy Manager tab to find the Docker host gateway. Set both runner bind addresses to that private IP and restart BaudBound.

Create a dynamic configuration file in the directory watched by Traefik's file provider:

```yaml
http:
  routers:
    baudbound-hooks:
      rule: "Host(`hooks.example.com`)"
      entryPoints:
        - websecure
      service: baudbound-hooks
      tls:
        certResolver: letsencrypt
    baudbound-socket:
      rule: "Host(`socket.example.com`)"
      entryPoints:
        - websecure
      service: baudbound-socket
      tls:
        certResolver: letsencrypt

  services:
    baudbound-hooks:
      loadBalancer:
        servers:
          - url: "http://host.docker.internal:43891"
    baudbound-socket:
      loadBalancer:
        servers:
          - url: "http://host.docker.internal:43892"
```

Traefik watches the file and supports WebSocket forwarding without a separate upgrade middleware. If Traefik runs directly on the host, replace `host.docker.internal` with `127.0.0.1` and keep the runner listeners on loopback. See Traefik's official [file provider documentation](https://doc.traefik.io/traefik/reference/dynamic-configuration/file/).

After completing a proxy tab, test the public routes:

```text
curl --fail-with-body \
  --request POST \
  --header 'Content-Type: application/json' \
  --data '{"message":"hello"}' \
  https://hooks.example.com/events/tutorial
npx wscat --connect wss://socket.example.com/events/messages
```

Use the method and path configured by the actual trigger. Confirm the related run and connection in BaudBound.

Before exposing either listener, decide how clients authenticate. Restrict source networks with the firewall or proxy. Set request and connection rate limits. Review body, message, and timeout limits. Keep untrusted request data out of sensitive logs.

## Failure guide

| Symptom | Likely cause | Check |
| --- | --- | --- |
| Connection refused | Listener stopped, disabled, wrong address, or wrong port | Service and Triggers views, Config, port ownership |
| `404` webhook response | No route matches method and hook name | Script enabled/approved, exact method, `/events/` path |
| `413` or oversized error | Request exceeds configured body limit | `max_body_bytes` and proxy body limit |
| `503` response | Executor unavailable, reloading, or at capacity | Service state and run concurrency logs |
| Webhook fallback response | Response node did not complete before timeout | Graph path, node failure, timeout setting |
| WebSocket handshake rejected | Unknown path or connection limit | Trigger path and `max_connections` |
| WebSocket write fails | Client disconnected or wrong connection ID | Use the ID from the current trigger output |
| Address already in use | Another process owns the bind address and port | Stop the duplicate service or select a deliberate new port |

Network access may require medium or high-risk capability review. Continue with [Approvals, Capabilities, and Risk](../security/approvals-capabilities.md) and [Runs, Logs, and Troubleshooting](runs-logs-troubleshooting.md).

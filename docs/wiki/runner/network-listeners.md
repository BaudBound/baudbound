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
| `127.0.0.1` | IPv4 loopback; reachable only from the same machine |
| `0.0.0.0` | Every IPv4 interface; potentially reachable from a LAN or wider network |
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

Requests larger than `max_body_bytes` are rejected before workflow execution. Invalid JSON does not remove the raw `body`; it leaves parsed JSON unavailable or empty according to runtime handling.

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

## Reverse proxy pattern

A reverse proxy can terminate TLS and restrict requests before forwarding them to a loopback listener. It is not authentication by itself.

This Nginx fragment is intentionally partial and belongs inside an already secured virtual host:

```nginx
location /events/ {
    proxy_pass http://127.0.0.1:43891;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
    client_max_body_size 1m;
}
```

For WebSockets, the proxy must also forward HTTP upgrade headers and use HTTP/1.1. Keep the runner on loopback unless direct LAN binding is an intentional and reviewed design.

Before external exposure, decide:

- how clients authenticate;
- which routes and methods are allowed;
- where TLS certificates terminate;
- which source networks the firewall permits;
- request and connection rate limits;
- body and message limits;
- timeout behavior; and
- which logs can contain untrusted request data.

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

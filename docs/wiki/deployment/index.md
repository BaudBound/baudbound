---
title: Deployment
description: Deploy the editor, schema host, and headless runner using the repository-supported methods.
tags: [deployment, operations]
---
# Deployment

## Editor container

The editor image is `ghcr.io/natroutter/baudbound-editor`. The repository compose file exposes port `3000`:

```text
docker compose -f apps/editor/compose.yaml pull
docker compose -f apps/editor/compose.yaml up -d
```

Set `NEXT_PUBLIC_EDITOR_URL` to the public HTTPS URL for a customized deployment. Point the reverse proxy to port `3000` and allow `.bbs` downloads. Pin a release or SHA image tag when the deployment must not follow `latest`.

## Schema host

The schema image is `ghcr.io/natroutter/baudbound-schemas`:

```text
docker compose -f deploy/schemas/compose.yaml pull
docker compose -f deploy/schemas/compose.yaml up -d
```

It binds `127.0.0.1:8085` and provides `/healthz`. Point the HTTPS reverse proxy for `schemas.baudbound.app` to `http://127.0.0.1:8085`. The runner validates with bundled contracts and does not require this server during execution.

## Headless runner

BaudBound does not install or manage an operating-system service. Configure the process supervisor used by the host to run:

```text
baudbound serve
```

Use a dedicated service account and persistent `BAUDBOUND_HOME`. When scripts use secrets, provide `BAUDBOUND_SECRET_KEY` through the host's secret manager. Grant the service account access only to required files, devices, and ports.

Before enabling automatic restart, run `baudbound doctor`, validate configuration, and import and approve required packages. Configure graceful termination and use the supervisor's own status and log commands. Script changes made through another CLI process reload automatically.

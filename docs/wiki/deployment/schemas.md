---
title: Schema Host Deployment
description: Publish the static BaudBound JSON Schema container behind a reverse proxy.
tags: [deployment, schemas]
---
# Schema Host Deployment

The schema image is built from the committed `schemas/` directory after CI verifies generated node schemas are current.

```text
docker compose pull
docker compose up -d
```

The provided compose configuration binds the container to `127.0.0.1:8085`. Point the HTTPS reverse proxy for `schemas.baudbound.app` to `http://127.0.0.1:8085`.

Publish an immutable commit or version tag and the `latest` convenience tag. Production compose files may pin an immutable tag for controlled rollout. Schema responses may be cached, but changed documents must not remain stale indefinitely; use explicit cache policy at Nginx and any CDN.

The runtime validates from bundled contracts and does not require the public schema host to execute packages. The host serves editor tooling, external validators, and cross-language integrations.

---
title: Schema Host Deployment
description: Run the published BaudBound JSON Schema container.
tags: [deployment, schemas]
---
# Schema Host Deployment

The schema image is published as `ghcr.io/natroutter/baudbound-schemas`. Start the repository compose file with:

```text
docker compose -f deploy/schemas/compose.yaml pull
docker compose -f deploy/schemas/compose.yaml up -d
```

It binds `127.0.0.1:8085` and provides `/healthz`. Point the HTTPS reverse proxy for `schemas.baudbound.app` to `http://127.0.0.1:8085`.

The schema server is not required for runner execution; the runner validates packages with bundled contracts.

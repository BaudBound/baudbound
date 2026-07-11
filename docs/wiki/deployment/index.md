---
title: Deployment
description: Self-host the BaudBound editor and schema service.
tags: [deployment, operations]
---
# Deployment

## Self-host the editor

The editor is stateless: it does not require a database or persistent container volume. Projects remain in the user's browser and exported `.bbs` packages are downloaded to the user's machine.

### Docker Compose

Create a directory for the deployment and add `compose.yaml`:

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

Replace `editor.example.com` with the public HTTPS hostname, then start it:

```text
docker compose pull
docker compose up -d
docker compose ps
```

`EDITOR_URL` controls canonical and social metadata in the published container. Pin a release or `sha-*` image tag instead of `latest` when updates must be promoted manually.

### Reverse proxy

Point the public HTTPS virtual host to `http://127.0.0.1:3000`. A minimal Nginx location is:

```nginx
location / {
    proxy_pass http://127.0.0.1:3000;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}
```

Configure DNS and a valid TLS certificate before publishing the site. Do not indefinitely cache HTML responses; hashed Next.js assets can use normal immutable caching. Updating the editor is:

```text
docker compose pull
docker compose up -d
```

## Schema host

The schema image is `ghcr.io/natroutter/baudbound-schemas`. The repository compose file binds it to `127.0.0.1:8085` and provides `/healthz`:

```text
docker compose -f deploy/schemas/compose.yaml pull
docker compose -f deploy/schemas/compose.yaml up -d
```

Point the HTTPS reverse proxy for `schemas.baudbound.app` to `http://127.0.0.1:8085`. Runner execution does not depend on the public schema server because package contracts are bundled with the runner.

To run scripts continuously on a headless Linux host, follow the dedicated [Linux Background Service](linux-background-service.md) guide.

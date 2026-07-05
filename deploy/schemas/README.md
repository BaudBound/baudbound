# BaudBound Schemas Container

This compose template runs the static schema host from the published GHCR image.

```bash
docker compose pull
docker compose up -d
```

The container listens on `127.0.0.1:8085`. Point the public reverse proxy for
`schemas.baudbound.app` to:

```text
http://127.0.0.1:8085
```

The image is built by `deploy/schemas/Dockerfile` from the committed `schemas/`
directory. CI verifies the generated node schemas are current before publishing
the container.

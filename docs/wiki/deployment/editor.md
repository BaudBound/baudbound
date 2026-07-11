---
title: Editor Deployment
description: Run the published BaudBound editor container.
tags: [deployment, editor]
---
# Editor Deployment

The editor image is published as `ghcr.io/natroutter/baudbound-editor`. The repository compose file uses the `latest` tag and exposes port `3000`:

```text
docker compose -f apps/editor/compose.yaml pull
docker compose -f apps/editor/compose.yaml up -d
```

Set `NEXT_PUBLIC_EDITOR_URL` to the public HTTPS URL before building or deploying a customized instance. Point the reverse proxy to the host's port `3000` and allow `.bbs` file downloads.

The `master`, commit SHA, release tag, and `latest` image tags are published by the Editor Docker workflow. Pin a release or SHA tag when deployment must not change after the next build.

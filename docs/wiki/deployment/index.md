---
title: Deployment
description: Deploy the BaudBound editor, schema host, documentation, and headless runner.
tags: [deployment, operations]
---
# Deployment

BaudBound's public services are independently deployable:

- The editor is a Next.js application published as a container.
- The schema host is a static Nginx container serving committed JSON Schemas.
- The wiki is a Wiki.js instance reconciled from `docs/wiki` by GitHub Actions.
- Headless runners are native processes managed by the operator's chosen supervisor.

Production deployments should use HTTPS, explicit image versions, health monitoring, backups, least-privilege service users, and controlled secrets. GitHub workflows build release artifacts and containers after their contract checks pass.

BaudBound does not automatically install a system service on user machines. Deployment templates may inform configuration, but the operator owns integration with systemd, OpenRC, runit, containers, or another process manager.

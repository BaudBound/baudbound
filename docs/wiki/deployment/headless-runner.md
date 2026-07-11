---
title: Headless Runner Deployment
description: Operate baudbound serve with a platform-appropriate process supervisor.
tags: [deployment, runner, headless]
---
# Headless Runner Deployment

Install the native `baudbound` binary, create a dedicated unprivileged service account, and choose a persistent `BAUDBOUND_HOME`. Generate and securely provision `BAUDBOUND_SECRET_KEY` when scripts need secrets.

Run `baudbound doctor`, validate configuration, import and approve packages, then configure the supervisor to execute `baudbound serve`. Set restart-on-failure with backoff, graceful termination, and a startup dependency on required network or device resources.

Protect the home directory because it contains installed automation, the SQLite state database, configuration, and encrypted secrets. Restrict serial devices, filesystem paths, and listener ports at the operating-system level in addition to BaudBound policy.

Use the supervisor's native status and log commands. The BaudBound CLI intentionally does not wrap systemctl or service commands. Script management from another CLI process is reflected by the running service through durable reload coordination.

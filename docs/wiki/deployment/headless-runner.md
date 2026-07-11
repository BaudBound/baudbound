---
title: Headless Runner Deployment
description: Operate baudbound serve with the system's process supervisor.
tags: [deployment, runner, headless]
---
# Headless Runner Deployment

BaudBound does not install or manage a system service. Configure the process supervisor used by the host to run:

```text
baudbound serve
```

Use a dedicated service account and persistent `BAUDBOUND_HOME`. When scripts use secrets, provide `BAUDBOUND_SECRET_KEY` through the host's secret manager. The service account needs access only to the files, serial devices, and ports required by approved scripts.

Before enabling automatic restart, run `baudbound doctor`, validate the configuration, and import and approve the required packages. Configure graceful termination and use the supervisor's own status and log commands.

Script import, update, enablement, and approval commands can run while the service is active; it reloads durable script changes automatically.

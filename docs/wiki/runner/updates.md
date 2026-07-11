---
title: Runner Updates
description: Check, download, verify, install, and restart signed BaudBound releases.
tags: [runner, updates]
---
# Runner Updates

Desktop builds check the signed update feed at startup. When a newer semantic version is available, the UI presents release information and lets the user decide whether to download it.

Download progress is shown in the update dialog. Before installation, Tauri verifies the artifact signature against the public updater key compiled into the application. The private signing key is held only by the release pipeline.

After a verified download, choose Restart to install and launch the new version. Windows updates use the installed application layout; Linux updates follow the supported artifact behavior for the installed format. Package-manager installations may additionally be governed by distribution tooling.

Update metadata is published as `latest.json` with each GitHub release. It maps platform targets to artifact URLs and signatures; it is not application configuration or user data.

Headless operators should follow the release notes and replace packages through their deployment process. Never install an artifact whose signature or expected release source cannot be verified.

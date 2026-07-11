---
title: Runs, Logs, and Runner Data
description: Find runner data and inspect execution history without exposing secrets.
tags: [runner, storage, logs]
---
# Runs, Logs, and Runner Data

Runner data is stored beneath `BAUDBOUND_HOME`, or the platform application-data directory when no override is set. Installed packages, approvals, run history, logs, and trigger state are managed by the runner. Configuration remains in `config.toml`.

Each run records script and trigger identity, start and completion time, terminal status, and error summary. Structured log entries include time, level, script, run, node, and message. Variable snapshots support run inspection but exclude secret plaintext.

Do not edit runner-managed state directly. Use runner commands or the desktop UI. Stop the runner before making a filesystem-level backup of its home directory.

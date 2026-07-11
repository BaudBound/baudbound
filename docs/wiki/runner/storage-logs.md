---
title: State, Runs, and Logs
description: Understand BaudBound durable SQLite state, installed packages, run history, and service status.
tags: [runner, storage, logs]
---
# State, Runs, and Logs

SQLite is the authoritative durable state for installed scripts, enablement, approvals, run records, logs, trigger checkpoints, and service coordination. Transactions preserve cross-table invariants and SQLite locking coordinates independent CLI, service, and Tauri command processes.

Installed `.bbs` files remain ordinary package files in the scripts directory. `config.toml` remains human-editable configuration. Sensitive values are encrypted before database storage. These formats have different responsibilities and should not be merged into one file.

Each run records script and trigger identity, start and completion time, terminal status, and error summary. Structured log entries include time, level, script, run, node, and message. Variable snapshots support run inspection but exclude secret plaintext.

Service health is durable runtime state consumed by CLI and UI status views. It is not the live control channel. Tauri IPC controls the desktop-owned service; supervised headless processes use operating-system lifecycle signals and durable reload coordination.

Do not edit the SQLite database manually. Use runner commands, the desktop UI, or supported backup tools while the database is safely quiesced.

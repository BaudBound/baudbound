---
title: Storage, Backups, and Recovery
description: Understand runner state and safely back up or restore a complete BaudBound runner home.
tags: [runner, storage, backup, recovery, sqlite]
---
# Storage, Backups, and Recovery

One runner home defines one logical BaudBound installation. Back up the complete runner home together with its external secret key before upgrades or machine migration. Do not edit the SQLite database manually.

## Runner home resolution

`BAUDBOUND_HOME` overrides the entire runner home. Without it, BaudBound uses:

| Platform | Default location |
| --- | --- |
| Windows | `%LOCALAPPDATA%\BaudBound\runner` |
| Linux with `XDG_DATA_HOME` | `$XDG_DATA_HOME/BaudBound/runner` |
| Other supported Linux setup | `$HOME/.local/share/BaudBound/runner` |

The desktop **Doctor** view and CLI status output show resolved paths. A service account has a different home from an interactive user unless both are deliberately configured with the same `BAUDBOUND_HOME`.

Never run two services against the same runner home. Although SQLite coordinates database writes, duplicated listeners can compete for ports, serial devices, files, and trigger events.

## Managed contents

| Path | Responsibility |
| --- | --- |
| `config.toml` | Machine-specific runner, listener, target, and serial configuration |
| `runner.sqlite3` | Durable runner state |
| `runner.sqlite3-wal` and `runner.sqlite3-shm` | SQLite write-ahead logging files that may exist while the database is open |
| `scripts/` | Validated copies of imported `.bbs` packages, retaining their imported filenames |

SQLite stores:

- installed script identity, package path, hash, versions, target, risk, and enabled state;
- per-revision approvals and approved permissions;
- completed run records, logs, and variable snapshots;
- service status and reload signals used between runner processes;
- script-scoped persistent variables;
- runner-global variables; and
- encrypted script secret values.

The database does not make encrypted secret values independently recoverable. Their encryption key is stored in the operating-system credential vault for desktop use or supplied through `BAUDBOUND_SECRET_KEY` for headless use.

## Supported backup method

Stop every BaudBound process that uses the runner home before copying it. This closes listener resources and allows SQLite to checkpoint its WAL state.

### 1. Stop the runner

For the desktop application, stop the background runner and choose **Quit BaudBound** from the tray menu. Confirm that the process has exited.

For a headless service, use the service manager's stop command and verify that status is inactive. See [Linux Background Service](../self-hosting/linux-background-service.md).

### 2. Copy the complete directory {.tabset}

#### Windows

Replace `PATH` with a destination outside the runner home:

```powershell
$source = Join-Path $env:LOCALAPPDATA "BaudBound\runner"
$destination = "PATH"
Copy-Item -LiteralPath $source -Destination $destination -Recurse
```

When `BAUDBOUND_HOME` is set, use that value as `$source` instead.

#### Linux

Replace `PATH` with a protected backup destination:

```bash
cp -a "$HOME/.local/share/BaudBound/runner" "PATH"
```

When `BAUDBOUND_HOME` or `XDG_DATA_HOME` is set, copy the resolved directory reported by BaudBound instead.

### 3. Back up the secret key

Desktop installations rely on the operating-system credential vault. Include the platform's supported credential-vault backup or machine-migration procedure; copying runner files alone does not copy that key.

Headless installations must back up the exact `BAUDBOUND_SECRET_KEY` through the operator's secret-management system. Do not place the plaintext key inside the same unencrypted archive as publicly accessible backups.

### 4. Verify the backup

Confirm that the copy contains `config.toml`, `runner.sqlite3`, and `scripts/`. Compare directory sizes and record the runner version used to create it. Protect the backup because it contains package behavior, logs, variable values, and encrypted secrets.

Restart the original runner only after verification.

## Restore

1. Install a runner version that supports the backup's database schema and package formats. The same version or a newer compatible release is safest.
2. Stop every BaudBound process on the destination.
3. Move any existing destination runner home aside; do not merge directories.
4. Copy the complete backup to the resolved destination path.
5. Restore ownership and permissions for the account that will run BaudBound.
6. Restore access to the original desktop vault key or headless `BAUDBOUND_SECRET_KEY`.
7. Start BaudBound and open **Doctor**, **Scripts**, **Security**, and **Service**.
8. Confirm schema health, package integrity, approval states, secret readiness, and listener registration before allowing unattended execution.

If the restored database schema is newer than the installed runner supports, stop and install the matching or newer release. Do not reduce the schema version manually.

## Why partial copies are unsupported

Copying only `runner.sqlite3` can omit active WAL content and installed package files. Copying only `scripts/` loses approvals, state, runs, and encrypted values. Mixing files from different backup times can make recorded hashes and package contents disagree.

Direct SQL edits bypass validation and can violate foreign keys, encryption requirements, package identity, variable versions, or approval invariants. There is no supported recovery procedure that begins by changing rows with a SQLite editor.

## Recovery symptoms

### Disk full

Stop the runner, free space on the volume containing the runner home, and confirm that the account can create files there. Restore from backup if the database or package copy was interrupted. Do not repeatedly restart a service that cannot write durable state.

### Permission denied

Confirm that the configured service account owns or can read and write the complete runner home. On Linux, inspect every parent directory as well as the files. Avoid making the directory world-writable.

### Database cannot open or reports corruption

Stop all runner processes and preserve a copy of the failed directory for diagnosis. Restore the latest known-good complete backup to a separate location. Do not run repair tools against the only copy.

### Packages fail integrity after restore

The database records and `scripts/` files likely came from different points in time or were modified. Restore them from one complete backup. Re-importing an authoritative package revision is safer than editing stored hash data.

### Secrets cannot decrypt

The runner does not have the original encryption key. Restore the credential-vault entry or headless key backup. If the key is permanently lost, encrypted values cannot be recovered; remove and set each secret again from its authoritative source.

### Newer schema error

The backup was opened by a newer BaudBound release. Install a compatible version. Downgrading a live runner home is unsupported unless release notes provide an explicit migration path.

## Data removal

Uninstalling the application does not necessarily remove the runner home. This protects scripts and state during reinstall.

Deleting the runner home permanently removes installed packages, approvals, run history, persistent and global values, and encrypted secret records. Stop BaudBound, create and verify a backup, and confirm the resolved path before deletion.

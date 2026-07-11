# baudbound-storage

SQLite-backed durable storage for the BaudBound runner.

The crate owns:

- Installed script metadata and controlled `.bbs` package references
- SHA-256 package integrity checks
- Script approvals bound to package hashes and permissions
- Completed and failed run records
- Service status snapshots
- Durable trigger-reload signals
- Versioned SQLite schema initialization

Package files remain under the runner-owned `scripts/` directory. All mutable runner metadata is stored in `runner.sqlite3`; the runner does not maintain parallel JSON indexes or logs.

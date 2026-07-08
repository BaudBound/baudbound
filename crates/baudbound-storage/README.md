# baudbound-storage

Filesystem-backed runner storage for installed script packages.

Current implementation:

- Imports validated `.bbs` packages into runner-owned storage
- Stores installed package bytes as `scripts/<original-package-file-name>.bbs`
- Persists an `index.json` with package metadata
- Supports list, find by id/name, update, enable/disable, hash verification, and remove
- Hashes imported packages with SHA-256
- Rejects unsafe script ids and unsafe package file names before creating filesystem paths
- Persists completed and failed run history to `runs.jsonl`
- Supports run history listing, script filtering, and limits

Planned responsibilities:

- Script permissions and approvals
- Runner settings


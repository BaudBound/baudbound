# baudbound-script

Script package and language model crate.

Planned responsibilities:

- `.bbs` package structures
- Manifest, program, permissions, and capabilities models
- JSON schema-aligned data types
- AST validation helpers

Current implementation:

- Reads `.bbs` zip packages
- Requires core package JSON files
- Rejects unexpected package files
- Validates asset package paths
- Requires zip asset files and `manifest.assets` entries to match


# baudbound-security

Security analysis and policy enforcement.

Current implementation:

- Maps known node `action_type` values to runner permissions and risk levels
- Recalculates required permissions from `program.json`
- Verifies `permissions.json` exactly matches the executable graph
- Loads the generated editor-owned node capability contract at compile time
- Recalculates required capabilities from `program.json`
- Verifies `capabilities.json` exactly matches the executable graph
- Rejects duplicate permissions, duplicate capabilities, and unknown action types
- Verifies declared package risk matches the recalculated highest risk
- Applies `RunnerPolicy` blocks for:
  - dangerous actions
  - shell commands
  - network server triggers

Runner lifecycle:

- Import/update uses a permissive policy to validate package truthfulness without blocking future approval flows
- Run uses the default restrictive policy before executing the graph

Planned responsibilities:

- Persisted user approvals
- Per-script and per-run allow/block policy

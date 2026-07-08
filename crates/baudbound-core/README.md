# baudbound-core

Core runner types and shared orchestration.

Current implementation:

- Package reader coordination
- Package import/update/remove/list orchestration
- Installed package hash verification before execution
- Manual and selected-trigger script execution through the runtime
- Core-level `action.script.run` handling for installed sub-scripts
  - runs the target script through its manual trigger
  - uses the same package hash verification, approval, permission, and run-history path as normal runs
  - rejects recursive sub-script cycles
- Trigger registration discovery and trigger event dispatch
- Headless action handler wiring for supported actions
- Custom action handler injection for desktop/runtime-specific action adapters
- Structured runner status reporting for CLI and future desktop UI
- Successful and failed run history persistence

Planned responsibilities:

- Logging abstractions
- Native desktop shell integration

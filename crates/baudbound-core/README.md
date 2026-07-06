# baudbound-core

Core runner types and shared orchestration.

Current implementation:

- Package reader coordination
- Package import/update/remove/list orchestration
- Installed package hash verification before execution
- Manual and selected-trigger script execution through the runtime
- Trigger registration discovery and trigger event dispatch
- Headless action handler wiring for supported actions
- Successful and failed run history persistence

Planned responsibilities:

- Logging abstractions
- Trigger lifecycle orchestration

# baudbound-triggers

Runner trigger contracts shared by CLI, daemon, and desktop runner shells.

Current implementation:

- Script-aware trigger registration model
- Trigger event model
- Trigger handler trait for listener registration
- Trigger dispatcher trait for delivering events into the runtime
- Schedule trigger service that parses schedule registrations and emits due trigger events
- Startup trigger service that queues one-shot runner-start events
- Webhook trigger service that matches HTTP requests, builds trigger payloads, and maps run output back to HTTP responses
- WebSocket trigger service that accepts routed WebSocket messages and keeps active connections available for write actions
- File watch trigger service that watches direct filesystem paths and emits file event payloads
- Hotkey trigger service that validates configured desktop key combinations, canonicalizes aliases, and builds dispatch payloads for native hotkey listeners
- Process started trigger service that polls for newly started matching processes
- Serial input trigger service that resolves script `deviceId` values through runner-side TOML serial devices and emits data payloads

Planned implementations:

- OS-native desktop hotkey listener integration on top of the shared hotkey service

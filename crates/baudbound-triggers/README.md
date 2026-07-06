# baudbound-triggers

Runner trigger contracts shared by CLI, daemon, and desktop runner shells.

Current implementation:

- Script-aware trigger registration model
- Trigger event model
- Trigger handler trait for listener registration
- Trigger dispatcher trait for delivering events into the runtime
- Schedule trigger service that parses schedule registrations and emits due trigger events
- Webhook trigger service that matches HTTP requests, builds trigger payloads, and maps run output back to HTTP responses
- File watch trigger service that watches direct filesystem paths and emits file event payloads
- Serial input trigger service that resolves script `deviceId` values through runner-side TOML serial devices and emits data payloads

Planned implementations:

- WebSocket
- Startup
- Desktop hotkeys


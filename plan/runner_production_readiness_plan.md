# Runner Production Readiness Plan

Snapshot date: 2026-07-11

This document tracks what must be completed before publishing the first production BaudBound runner release. The editor is considered mostly ready feature-wise, so this plan focuses on making the Rust runner support every editor-exported feature, or explicitly reject unsupported platform combinations before execution.

## Release Goal

Publish the first production runner release when:

- Every editor node has a corresponding runner implementation, validation path, and platform support rule.
- Every unsupported platform/action combination is blocked in both the editor and runner.
- Runner import, approval, storage, trigger, and execution behavior is covered by automated checks where practical.
- The desktop runner can be installed and used by normal users.
- The headless runner can be used manually with clear deployment instructions.
- No feature is half-implemented, silently ignored, or allowed to fail only at runtime when it could be rejected earlier.

## Long-Term Runner Architecture

The first production release should start on the long-term architecture instead of shipping short-term coordination files that must be replaced later.

Required architecture:

- SQLite is the durable runner state store.
- IPC is the live runner control channel.
- Tauri commands are the desktop UI bridge.

SQLite owns durable state:

- Installed script metadata.
- Package hashes and package file references.
- Approvals.
- Run history and run logs.
- Service/background-runner status snapshots.
- Durable reload signals where a persisted signal is appropriate.
- Per-script persistent variables and runner-wide global variables, with optimistic concurrency control.
- Authenticated-encrypted secret values. Secret plaintext is never stored in SQLite.
- Future config migration metadata.

Variable and secret ownership:

- `runtime` variables exist only for one execution.
- `persistent` variables are writable script-owned values retained across executions.
- `global` variables are writable runner-owned values shared across scripts and require an elevated permission.
- Secrets are runner-managed, read-only script inputs. Packages contain declarations only: name, type, description, and whether the value is required.
- Desktop secret encryption uses a random key held by the operating-system credential vault.
- Headless secret encryption requires `BAUDBOUND_SECRET_KEY`; there is no plaintext or automatically persisted key fallback.
- The editor may accept real secret values for simulation only when the user enters them for that session. Simulation values are never written into editor state, autosaves, or exported packages, and simulation logs/snapshots redact them.
- Secret values do not expose derived metadata variables such as `$length`, `$count`, `$type`, or `$is_empty`.

IPC owns live control:

- Stop running service/background runner.
- Reload trigger registrations.
- Query live runner status when a runner process is active.
- Future live commands that should not be represented as files.

Long-running trigger execution must not block the service control loop. Network trigger hosts use bounded execution queues and fixed worker pools so reload, stop, heartbeat, and unrelated requests remain responsive under load. Client response deadlines are coordination deadlines: a timed-out workflow continues to completion and its run result remains durable until explicit runtime cancellation semantics are implemented.

Tauri commands own desktop UI communication:

- React UI must call Tauri commands for runner operations.
- Tauri commands may read/write SQLite and communicate with the live runner through IPC.
- The desktop UI must not depend on service-control JSON files as a production control channel.

Files remain valid only for file-shaped data:

- `.bbs` packages and package assets.
- User-editable TOML config.
- Optional exported diagnostics/log bundles.
- Human-authored deployment templates and documentation.

The runner has not been publicly released with an older storage format, so no legacy JSON migration is required. Obsolete JSON storage and control implementations have been removed. Runner operation uses SQLite plus IPC and does not maintain parallel metadata formats.

## Current Readiness Snapshot

| Area | Current estimate | Notes |
| --- | ---: | --- |
| Runtime execution engine | 97% | Control flow, all writable variable scopes, read-only secret inputs, derived metadata, and the complete documented Calculate expression contract have focused coverage. Cooperative cancellation policy remains. |
| Action coverage | 93% | Text, calculation, HTTP, filesystem, process, and shell behavior have focused coverage. Process window-title query/termination now use native Win32 APIs and are rejected for every non-Windows-Desktop target. Remaining work is concentrated in native desktop verification and final platform review. |
| Trigger services | 99% | Webhook, WebSocket, File Watch, process-start, and schedule lifecycle/reload behavior have focused concurrency coverage. Process-start uses stable process identities and acknowledged in-place reload; schedules preserve cadence and coalesce missed ticks. Final serial/hotkey platform verification remains. |
| Serial/device system | 80% | Logical device IDs, reconnect, USB identity validation, port rebinding, scanner, and UI exist. Codex-owned work focuses on config/import/UI correctness; physical multi-device validation is user-owned for now. |
| Security/approval/storage | 99% | Permissions/capabilities are independently recalculated, package/update/hash policy is covered, durable variables use SQLite CAS, and secrets use authenticated encryption with vault/environment key ownership and report redaction. Desktop approval UI reliability remains. |
| Desktop UI | 80% | Main workflows use SQLite-backed state, live trigger reload uses authenticated IPC, and declared secrets can be configured without returning values to React. Final polish, error states, packaging validation, and production install behavior remain. |
| Codebase maintainability | 99% | Domain code and focused suites are modular, the editor generates the runner capability contract from node definitions as the single source of truth, trigger execution uses one tested bounded executor, and File Watch, WebSocket, process-start, and schedule ownership are split into focused modules. A few older broad fixture/test modules remain candidates for later splitting. |
| Packaging/release | 40% | Initial runner CI quality gate exists. Still needs Tauri release packaging, GitHub Releases updater artifacts, Linux AppImage, versioning, signing decisions, and documentation. |
| Cross-platform native support | 56% | First release supports Windows and Linux only. Windows now has native window-title process actions and trigger matching with precise config-sensitive gating; Linux desktop support still needs verification or precise rejection for remaining native features. |
| Overall first production runner readiness | 93% | SQLite/IPC, package truthfulness, all variable lifecycles, encrypted secrets, file/network/process behavior, Sub-script approval boundaries, and major trigger lifecycle/reload behavior are substantially covered. Release packaging, cancellation, platform verification, and final desktop polish remain. |

## Feature Coverage Baseline

The runner must support or intentionally reject every feature exported by the editor.

### Control Flow

| Editor node | Runner status | Production requirement |
| --- | --- | --- |
| If / Else | Implemented and matrix-tested | All exported comparison operators, malformed numeric/regex conditions, and inversion are covered. |
| Switch | Implemented and tested | Matching, no-match branch termination, and both exported case value field shapes are covered. |
| Loop | Implemented and tested | The body executes for the configured count without returning to the loop input, then follows `done`. |
| While | Implemented and tested | Iterative and false-first behavior are covered. Long-running soak validation is user-owned. |
| For Each | Implemented and tested | JSON lists, nested variable paths, empty lists, and non-list rejection are covered. |

### Runtime/Data Actions

| Editor node | Runner status | Production requirement |
| --- | --- | --- |
| Variable Operation | All writable scopes implemented and tested | Runtime, per-script persistent, and runner-wide global scopes support the exported operations. Stored writes use versioned compare-and-set updates. Secrets are intentionally separate read-only manifest declarations and cannot be selected as a writable scope. |
| Calculate | Implemented and matrix-tested | Every documented operator/function, precedence, scientific notation, malformed expression, non-finite result, and random range is covered. |
| Format Text | Implemented and matrix-tested | All 17 editor operations, exported join-list strings, and malformed regex/Base64/URL/JSON/list inputs are covered. |
| Log | Implemented | Ensure logs appear consistently in CLI and desktop UI. |
| Delay | Implemented | Add cancellation/stop behavior decision for long delays. |

### File, Network, Process, and System Actions

| Editor node | Runner status | Production requirement |
| --- | --- | --- |
| HTTP Request | Implemented and matrix-tested | All editor methods, body policy, list/object headers, timeout, connection/config failures, JSON output, and multi-megabyte responses are covered. |
| Download File | Implemented and tested | Success, HTTP failure, overwrite protection, overwrite replacement, destination creation, and output metadata are covered. |
| Read File | Implemented and tested | UTF-8 success, unsupported encoding, invalid UTF-8, missing paths, and output byte counts are covered. |
| Write File | Implemented and tested | Append, overwrite, parent creation, invalid modes, and invalid destination types are covered. |
| Delete File | Runtime behavior tested | Regular-file deletion, missing paths, and directory rejection are covered. Dangerous approval flow remains in the security audit. |
| Copy File | Implemented and tested | Overwrite policy, missing/invalid paths, same-file protection, parent creation, byte counts, and data preservation are covered. |
| Move File | Implemented with native platform backends and tested | Linux uses native rename; Windows uses `MoveFileExW` with replace/cross-volume flags. Overwrite, missing/invalid paths, same-file protection, and source preservation are covered. |
| Run Process | Implemented and tested | Arguments, quoted values, Windows paths, working directory, exit code, stdout/stderr, missing executables, and invalid working directories are covered. The current editor contract is wait-and-capture only; it does not export a wait/no-wait option. |
| Process Status | Implemented and tested | PID/name/path matching is deterministic and platform-correct. Window-title mode uses native Win32 enumeration on Windows Desktop; editor verification and runner import reject that mode for generic, Linux, and headless targets. Safe native not-found behavior is automated; real-window validation is user-owned. |
| Kill Process | Implemented and tested | PID/name/path use the shared process backend. Window-title mode uses native Win32 enumeration plus `TerminateProcess`, selects the lowest matching PID deterministically, and is gated to Windows Desktop. Real-window termination validation is user-owned. |
| Shell Command | Implemented and tested | Nonzero exit, stdout/stderr capture, dangerous classification, and the independent shell policy gate are covered. |
| Sub-script | Implemented and tested | Successful child execution, persisted child run linkage, missing child failure, recursion prevention, and independent child approval enforcement are covered. |

### Desktop Actions

These must use native APIs only. If a platform has no native backend, the editor and runner must reject the package for that platform.

| Editor node | Runner status | Production requirement |
| --- | --- | --- |
| Clipboard | Implemented via native clipboard backend | Verify Windows and Linux desktop behavior or gate per platform. |
| Keyboard / Press Key | Implemented via native input backend | Verify required OS permissions and failure messages. |
| Type Text | Implemented via native input backend | Verify text layout, modifiers, and non-ASCII behavior. |
| Mouse Click | Implemented via native input backend | Verify buttons on Windows and Linux. |
| Move Mouse | Implemented via native input backend | Verify relative/absolute movement. |
| Notification | Implemented via native notification backend | Verify desktop notification service behavior per OS. |
| MessageBox | Implemented via native dialog backend | Verify button results and modal behavior. |
| Play Sound | Implemented via native audio backend | Verify file path and packaged asset playback. |
| Beep | Implemented as terminal bell | Decide whether terminal bell is acceptable production behavior on desktop, or add native OS beep where available. |
| Get Pixel Color | Windows Desktop only | Verify Win32 behavior; block Linux until a native backend exists. |
| Get Active Window | Windows Desktop only | Verify Win32 behavior; block Linux until a native backend exists. |
| Window Focus | Windows Desktop only | Verify Win32 behavior; block Linux until a native backend exists. |
| Open Application | Implemented | Audit whether current implementation is native enough per platform, and gate where not. |

### Triggers

| Editor trigger | Runner status | Production requirement |
| --- | --- | --- |
| Manual | Implemented | Must work from CLI and desktop UI. |
| Schedule | Implemented and timing-tested | Fractional intervals, strict duration bounds, unchanged/changed reload behavior, drift-free cadence, and coalesced missed intervals are covered. |
| File Watch | Implemented and lifecycle-tested | Static paths, file deletion/recreation, normalized create/modify/delete/rename events, optional recursive directory watching, burst delivery, and registration replacement are covered. |
| Webhook | Implemented and concurrency-tested | Bounded worker execution, overload rejection, immediate responses, response-node success, configured deadlines/fallback, missing responses, body limits, headers, method routing, dispatch failures, and active route reloads are covered. Timed-out runs intentionally continue until runtime cancellation semantics exist. |
| WebSocket | Implemented and concurrency-tested | Real loopback tests cover concurrent clients, text/JSON/binary payloads, handshake headers/query data, server writes, unique connection IDs, unknown routes, connection limits, protocol-level message limits, disconnect cleanup, shutdown, and route reload without listener rebinding. |
| Serial Input | Implemented | Add automated coverage for config, reconnect decisions, auto-rebind decisions, identity matching, and line/raw modes where practical. Physical multi-device validation is user-owned for now. |
| Hotkey | Implemented | Verify native hotkey support and OS permission requirements. |
| Startup | Implemented as runner-start trigger | Document that headless startup is controlled by user service manager; desktop app startup behavior still needs release decision. |
| App/Process Started | Implemented and lifecycle-tested with platform gating | Name/path matching remains cross-platform. Window-title matching uses native Win32 enumeration only for Windows Desktop. Stable process identities, duplicate suppression, PID reuse, pending-window preservation, reload boundaries, worker reuse, and prompt shutdown are covered. |

## Blocking Work Before First Release

### 1. Complete the Editor-to-Runner Feature Matrix

Create and maintain a table that lists every editor node and trigger with:

- Editor action type.
- Runner type.
- Required permission.
- Required capability.
- Supported target runtimes.
- Runner implementation module.
- Automated test coverage.
- Manual test coverage.
- Known platform limitations.

Done when:

- The matrix covers all editor node definitions.
- CI fails if an editor node lacks runner metadata or runner support.
- Unsupported platforms are visible in both editor verification and runner import errors.

### 2. Native Desktop Action Verification

Run real desktop tests, especially on Windows Desktop first.

Required Windows checks:

- Clipboard set/read behavior through user-visible clipboard.
- Notification appears and failure is reported clearly when notifications are unavailable.
- MessageBox shows correct type/buttons and returns the selected button.
- Keyboard press and Type Text work in a normal focused text field.
- Mouse Click and Move Mouse work with relative and absolute movement.
- Play Sound works with file paths and packaged assets.
- Get Pixel Color returns correct color formats.
- Get Active Window returns title/process/process id.
- Window Focus focuses by supported match modes.
- Open Application works without shell-script hacks.

Done when:

- Every native action has at least one manual verification note.
- Every unsupported native action has a deterministic import/export rejection path.
- The `doctor` command accurately reports native backend availability.

### 3. Headless Runner Hardening

Headless mode must be production-usable without desktop assumptions.

Required:

- Headless target runtimes reject desktop-only packages.
- Service/serve command reloads triggers correctly after import/update/remove/enable/disable.
- CLI commands work while a serve process is running.
- Live service control uses IPC instead of service-control JSON files.
- Service status snapshots are stored durably in SQLite.
- Long-running serve process can be stopped cleanly.
- Logs are readable and bounded enough to avoid unbounded growth issues.

Done when:

- Import/update/remove workflows are documented and verified.
- No desktop UI dependency exists for headless use.
- Automated reload and status/control checks cover expected headless command interactions.

### 4. Trigger Reload and Race Hardening

Codex-owned automated tests:

- [x] Webhook concurrent requests.
- [x] Webhook wait-for-response success, timeout, and missing response behavior.
- [x] WebSocket connect/message/disconnect and write response.
- [x] File watch with rapid changes.
- [x] Process-started duplicate suppression.
- [x] Trigger reload while events are arriving.

Done when:

- Trigger services handle reloads and concurrent events in automated tests.
- Failed trigger events create understandable logs.
- A script update cannot leave old trigger registrations active.

User-owned validation outside this plan:

- Long-running soak tests.
- Physical serial disconnect/reconnect/rebind behavior.
- Multi-device serial port swap behavior.

### 5. Serial Device Config and UI Production Readiness

The runner and desktop UI must make serial configuration safe and understandable without requiring script packages to store local port details.

Required:

- Device ID in editor maps to runner TOML device config.
- Serial input and serial write use the same logical device ID.
- USB vendor/product validation is represented in config, UI, and import/runtime errors.
- Auto rebind config requires enough USB identity information before it can be enabled.
- Auto rebind config updates are visible in logs/UI when the runner changes a port.
- Serial number/manufacturer/product matching is documented.
- Devices tab scanner creates valid config entries.

Done when:

- Config update behavior is visible and understandable in UI/logs.
- Invalid serial config fails safely with clear errors.
- Physical multi-device tests are tracked separately by the user.

### 6. Security and Approval Final Audit

Required:

- Installed script metadata, approvals, run records, and package hashes are stored in SQLite.
- Package hash mismatch blocks execution.
- Updated packages invalidate approval.
- Dangerous/high/medium permissions are visible and require approval where intended.
- Permissions are recalculated by runner and compared to package declarations.
- Capabilities are recalculated by runner and compared to package declarations.
- Minimum runner version is enforced.
- Tampered `.bbs` packages fail import or execution safely.
- Sub-scripts cannot bypass approval. Completed 2026-07-10.
- Shell and process-kill permissions are clearly high-risk/dangerous.
- Persistent/global writes and secret reads have configuration-derived permissions and capabilities.
- Secret values are authenticated-encrypted at rest and never returned by CLI status or Tauri dashboard commands.
- Runtime reports, logs, errors, and editor simulation snapshots redact secret values.
- Required secret declarations block execution when no value is configured.

Done when:

- A small abuse-case suite exists. Completed 2026-07-10.
- Desktop UI approval modal works reliably.
- CLI approval commands remain complete for headless users.

### 7. Desktop UI Release Polish

Required:

- Scripts tab: import/update/remove/run/approve/revoke flows are clear.
- Runs tab: logs and variables are readable for failed and successful runs.
- Config tab: simple and advanced modes both work, with TOML validation.
- Devices tab: scanner and add-to-config flow work reliably.
- Security tab: approval/risk/package hash language is user-friendly.
- Security tab: declared secrets can be configured, replaced, and removed without revealing stored values.
- Service tab: background runner state is accurate and not confused with external service management.
- Doctor tab: shows real diagnostics, not placeholder/config duplication.
- No horizontal scrolling in normal responsive layouts.
- Toasts/errors are consistent.
- Desktop commands use Tauri as the UI bridge and do not rely on JSON service-control files for live runner control.

Done when:

- App can be resized down to common laptop sizes without broken layouts.
- Every command error is shown in a useful way.
- UI can manage the full first-release runner workflow.

### 8. Packaging and Distribution

Required:

- Decide version number for first runner release.
- Finalize Tauri build config.
- Build Windows desktop installer as the primary Windows artifact.
- Use a Windows installer by default because it gives normal users shortcuts, registration in Apps & Features, a clean uninstall path, and a clearer trust/install flow than a bare portable executable.
- Keep a portable Windows executable as optional future work, not the first-release baseline.
- Build Linux AppImage as the primary cross-distro desktop artifact for Debian, Fedora, Arch, and other major distributions.
- Build headless/CLI binary artifacts where useful, but keep the desktop AppImage as the main Linux desktop distribution path.
- Decide code signing strategy.
- Decide whether Windows code signing is required for the first public release or documented as unsigned early release behavior.
- Add release CI jobs for Rust, desktop UI, Tauri packages, schemas, and examples.
- Add GitHub Releases publishing for all first-release artifacts.
- Generate Tauri updater artifacts and signatures during release builds.
- Publish a GitHub Releases updater manifest, such as `latest.json`, with signed artifact URLs.
- Configure the desktop app to check for updates on startup.
- Add a desktop update modal that shows the available version, release notes summary, download progress, and a restart button after install.
- Use Tauri's updater flow for desktop updates instead of a custom tag-only downloader, so signatures and platform artifacts are verified before installation.
- Keep headless update behavior separate from desktop auto-update. A later `baudbound update` command is acceptable for CLI/headless, but it is not required for the first desktop auto-update milestone.
- Document install/update/uninstall.
- Confirm first-run config initialization.
- Confirm app icon/name/metadata.

Done when:

- A clean machine can install and run the desktop app.
- A clean headless machine can run the CLI/serve workflow from docs.
- Release artifacts are reproducible from CI.
- Windows desktop release installs through a standard setup/installer package.
- Linux desktop release runs from AppImage without requiring distro-specific package installation.
- Desktop app can detect, download, install, and relaunch into a newer GitHub Release build.
- Update signatures are validated before install.

Windows release decision:

- The first Windows desktop release should ship an installer as the default artifact.
- The installer should be built from the Tauri release pipeline on a Windows CI runner.
- The installer is preferred over a bare executable for production because it provides shortcuts, app registration, uninstall behavior, and a familiar install experience.
- A portable executable can be added later if users specifically need it.

Linux release decision:

- The first Linux desktop release should ship an AppImage as the default universal artifact.
- `.deb` and `.rpm` packages can be added later, but they are not the first-release baseline because they create distro-specific install and update behavior.
- Build AppImage on a conservative Linux baseline to avoid avoidable glibc incompatibility on older supported distros.
- User-facing install instructions should be simple: download the AppImage, make it executable if required by the desktop environment, and run it.

Desktop updater target behavior:

- On app start, the desktop UI checks GitHub Releases for a newer signed release.
- If an update exists, the UI opens a clear update modal instead of silently updating.
- The modal lets the user start the update, shows download progress, and shows a restart button after installation is ready.
- Restart launches the updated app.
- Failed update checks or downloads must be non-fatal and visible in the UI/logs.
- The updater must never run unsigned or mismatched artifacts.

### 9. Documentation Required for First Release

Required docs:

- Runner quick start.
- Import/update/remove/list/run scripts.
- Approval and package hash model.
- Headless serve mode.
- Example systemd/service-manager templates, without automatic installation.
- Desktop app usage.
- Serial device config.
- Webhook/WebSocket trigger setup.
- Target runtimes and platform limitations.
- Native action support table.
- Troubleshooting and `doctor`.

Done when:

- A new user can install the runner, import a package, approve it, run it, and debug common failures without reading source code.

### 10. CI and Quality Gates

Required:

- Rust `cargo fmt`.
- Rust `cargo clippy -- -D warnings`.
- Rust tests for all runner crates.
- Editor package contract tests.
- Schema generation check.
- Desktop UI typecheck/build.
- Tauri build check.
- Example package validation.
- Optional release artifact smoke test.
- SQLite schema initialization and upgrade tests.
- IPC live-control tests.

Done when:

- CI is green on a clean checkout.
- CI covers the runner/editor contract so future editor nodes cannot be added without runner awareness.

### 11. Codebase Modularity and Maintainability

The runner crates must not ship as giant single-file crates. Large files make code review, ownership, testing, and production debugging unnecessarily risky.

Required:

- Split inline crate tests into crate-local `tests.rs` modules or focused integration tests.
- Split large production files by stable domain boundaries instead of using `include!`-style file dumps.
- Do not let crate `src` folders become flat piles of unrelated modules. Use clear domain folders such as `services/`, `actions/`, `runtime/`, `storage/`, or similar ownership-based folders when a crate grows beyond a few modules.
- Keep public crate APIs small and intentional through `lib.rs` re-exports.
- Prefer modules such as action domains, trigger services, runtime graph/expression/evaluator modules, storage index/run/approval modules, and core orchestration/status/trigger/sub-script modules.
- Avoid new source files growing past roughly 700-900 lines unless there is a strong reason.
- Keep module visibility tight with `pub(crate)` and explicit public re-exports.
- Preserve behavior during structural moves with focused tests and clippy.

Done when:

- No runner crate production file is a broad mixed-responsibility file over 1000 lines.
- `baudbound-actions`, `baudbound-runtime`, `baudbound-triggers`, `baudbound-core`, and `baudbound-storage` have clear module folders/files.
- Tests remain green after each split.
- Future feature work has obvious module homes.

### 12. SQLite and IPC Production Architecture

Required:

- Store installed script metadata, approvals, run history, service status, and durable reload signals in SQLite.
- Use authenticated IPC for live stop/reload requests.
- Keep a single production storage implementation; no unpublished JSON backend or migration path is required.
- Keep `.bbs` packages as files under a controlled packages directory and store only metadata/hash/path references in SQLite.
- Keep TOML config as the user-editable config source unless a future release intentionally moves config into the database.
- Add tests for schema initialization and upgrades, state round-trips, and IPC stop/reload behavior.

Done when:

- New runner homes create a SQLite database automatically.
- CLI, serve mode, desktop UI, and tests use SQLite-backed durable state.
- Live stop/reload control no longer uses JSON files.
- Obsolete JSON control/state code is removed.

## Missing or Weak Areas To Investigate

These are not confirmed blockers yet, but must be reviewed before release:

- Whether terminal bell is acceptable for Beep, or whether native OS beep APIs are required.
- Whether Open Application uses acceptable native behavior on each supported desktop platform.
- Whether process/window title matching is available outside desktop contexts and gated correctly.
- Whether Linux desktop native input/notification behavior works under Wayland, X11, or both.
- Whether package/run logs need retention limits or cleanup controls.
- Whether minimum runner version should be bumped before first release.
- Whether runner config migration is needed for future changes.
- Whether old packages using removed target runtimes should be rejected with a clear unsupported-platform message.
- Whether the authenticated loopback IPC transport should move to platform-specific named pipes/Unix sockets in a later release. The first-release transport is now fixed as authenticated local TCP with bounded typed messages.

## First Release Exit Criteria

The first production runner release can be published only when all of these are true:

- [ ] Every editor node/trigger is implemented or explicitly blocked per platform.
- [ ] Editor verification and runner import agree on platform compatibility.
- [ ] All required automated test suites pass in CI.
- [ ] Native desktop action support is either implemented and automated where possible, or gated with clear unsupported-platform errors.
- [ ] Headless serve mode passes automated command/reload/status checks.
- [ ] Serial device config and UI flows are verified automatically where possible.
- [x] Webhook/WebSocket trigger tests pass under concurrency.
- [ ] Package hash and approval flows are verified in CLI and desktop UI.
- [ ] Desktop UI can complete import, approve, run, view logs, edit config, scan devices, and manage background runner state.
- [x] Durable state uses SQLite.
- [x] Live runner control uses IPC instead of JSON control files.
- [ ] Release packages can be built from a clean checkout.
- [ ] First-release documentation exists.

User-owned release validation outside this Codex execution plan:

- Real-machine native desktop action checks.
- Long-running soak tests.
- Physical serial hardware tests, including multi-device reconnect/rebind behavior.

## Suggested Execution Order

1. Build the editor-to-runner feature matrix and use it to find exact gaps.
2. Close missing implementation gaps or add precise platform rejections.
3. Migrate durable runner state to SQLite. Completed 2026-07-10.
4. Replace live service-control JSON files with IPC. Completed 2026-07-10.
5. Add automated tests for graph execution, actions, triggers, package validation, storage, and IPC. Completed for the first-release automated matrix on 2026-07-10; user-owned physical and native desktop checks remain separate.
6. Harden headless serve/reload workflows.
7. Finish desktop UI release polish.
8. Add release packaging and CI.
9. Write first-release docs.
10. Run final clean-machine release rehearsal.

## Completed Work Log

### 2026-07-09 Editor-to-Runner Support Ownership Gate

Completed:

- Added explicit runner support ownership constants for action handlers, runtime-owned actions, runtime control flow, core-routed actions, core-routed manual triggers, and service-backed triggers.
- Added an editor package-contract test that compares every editor node action type against the Rust runner owner lists.
- Made CI fail when a new editor executable action, control node, or trigger is added without a corresponding runner owner.
- Kept target-runtime compatibility policy checks separate, so support ownership and platform availability are both tested.

Validation:

- Ran editor package contract tests.
- Ran Rust tests for `baudbound-actions`, `baudbound-core`, `baudbound-runtime`, and `baudbound-triggers`.
- Ran clippy with warnings denied for the same Rust crates.

### 2026-07-09 Runner Crate Modularity Pass 1

Completed:

- Moved inline tests out of the large runner crate `lib.rs` files into crate-local `tests.rs` modules.
- Extracted `baudbound-actions` serial device configuration, serial port construction, and USB identity validation into `baudbound-actions/src/actions/serial.rs`.
- Extracted `baudbound-actions` text formatting operations into `baudbound-actions/src/actions/text.rs`.
- Moved extracted `baudbound-actions` action implementation modules under `baudbound-actions/src/actions/`.
- Split `baudbound-actions` file actions into `baudbound-actions/src/actions/files.rs`.
- Split `baudbound-actions` HTTP and webhook-response actions into `baudbound-actions/src/actions/network.rs`.
- Split `baudbound-actions` process, shell, and open-application actions into `baudbound-actions/src/actions/process.rs`.
- Split `baudbound-actions` beep and desktop-only fallback helpers into `baudbound-actions/src/actions/system.rs`.
- Extracted the `baudbound-runtime` calculation expression tokenizer/parser/evaluator into `baudbound-runtime/src/runtime/calculation.rs`.
- Extracted `baudbound-runtime` config field readers into `baudbound-runtime/src/runtime/config.rs`.
- Extracted `baudbound-runtime` control-flow frame/config row structs into `baudbound-runtime/src/runtime/control.rs`.
- Extracted `baudbound-runtime` condition comparison helpers into `baudbound-runtime/src/runtime/conditions.rs`.
- Extracted `baudbound-runtime` program graph parsing/navigation into `baudbound-runtime/src/runtime/graph.rs`.
- Extracted `baudbound-runtime` template/config resolution helpers into `baudbound-runtime/src/runtime/templates.rs`.
- Extracted `baudbound-runtime` variable operations, derived metadata, value coercion, and typed conversion helpers into `baudbound-runtime/src/runtime/variables.rs`.
- Extracted `baudbound-core` serial device config projection into `baudbound-core/src/serial.rs`.
- Extracted `baudbound-core` runner/script status DTOs, trigger status labels, and approval status comparison into `baudbound-core/src/status.rs`.
- Extracted `baudbound-core` trigger dispatcher and package trigger-registration extraction into `baudbound-core/src/triggers.rs`.
- Extracted `baudbound-core` package inspection, import request construction, and permission projection into `baudbound-core/src/package.rs`.
- Extracted `baudbound-core` run-record conversion and failed-run persistence helpers into `baudbound-core/src/run_records.rs`.
- Extracted `baudbound-core` sub-script action dispatch and recursion-safe child-run handling into `baudbound-core/src/sub_script.rs`.
- Extracted `baudbound-storage` script id validation, package filename validation, package hashing, atomic writes, safe package deletion, directory creation, file copy, and timestamp helpers into `baudbound-storage/src/storage/filesystem.rs`.
- Extracted `baudbound-storage` storage and approval metadata index structs into `baudbound-storage/src/storage/metadata.rs`.
- Extracted `baudbound-storage` approval read/write/approve/revoke lookup logic into `baudbound-storage/src/storage/approvals.rs`.
- Extracted `baudbound-storage` run history append/list/sort/filter logic into `baudbound-storage/src/storage/runs.rs`.
- Extracted the schedule trigger service into `baudbound-triggers/src/services/schedule.rs`.
- Extracted startup, file-watch, process-started, and WebSocket trigger services into focused trigger modules.
- Extracted hotkey normalization/dispatch into `baudbound-triggers/src/services/hotkey.rs`.
- Extracted serial input reader configuration, reconnect/rebind logic, status tracking, and USB identity matching into `baudbound-triggers/src/services/serial_input.rs`.
- Extracted webhook routing, payload creation, and response-from-report logic into `baudbound-triggers/src/services/webhook.rs`.
- Moved trigger service implementation modules under `baudbound-triggers/src/services/` so the crate root contains contracts, exports, shared helpers, and tests only.
- Added modularity and maintainability as an explicit production-readiness requirement.

Still remaining:

- Split the large `baudbound-triggers` serial input module into smaller serial config, status, USB identity, and reader-loop modules if it continues to grow.
- Split remaining `baudbound-actions` serial write and WebSocket write dispatch out of the handler root if those areas grow further.
- Keep `baudbound-runtime` executor dispatch in the crate root for now; split into an executor module if the current 839-line root grows again or gains unrelated responsibilities.
- Keep `baudbound-core` orchestration focused; split further only when new responsibilities would make the root harder to review.
- Consider extracting large test fixture builders from crate-local `tests.rs` files if test maintenance becomes a bottleneck.

Validation:

- Ran focused tests for the refactored action/runtime/trigger crates during the split.
- Ran broad runner crate tests for actions, core, runtime, security, storage, and triggers after the runtime split.
- Ran clippy with `-D warnings` for actions, core, runtime, security, storage, and triggers after the runtime split.
- Ran editor package/schema tests to keep editor-to-runner contracts aligned.

### 2026-07-09 Minimum Runner Version Enforcement

Completed:

- Added runner-side `minimum_runner_version` parsing and comparison.
- Accepted normal `MAJOR.MINOR.PATCH` values and release-tag-style leading `v` values.
- Rejected invalid package minimum runner versions.
- Rejected packages that require a newer runner during package validation and import.
- Rejected already-installed packages that require a newer runner during execution.
- Blocked trigger registration for already-installed packages that require a newer runner.
- Reported too-new installed packages through runner status instead of hiding the problem.

Validation:

- Added unit tests for version parsing/comparison.
- Added core tests for validate/import/run/trigger-registration/status paths.

### 2026-07-09 Runner CI Quality Gate

Completed:

- Added `.github/workflows/runner-ci.yml` for runner production quality checks.
- The workflow runs Rust formatting, clippy with warnings denied, full workspace tests, desktop UI typecheck/build, editor schema checks, and editor contract tests.
- The Rust job runs on both Windows and Linux so platform-specific runner code is compiled and tested on both first-release target families.
- The Linux CI job installs the desktop build dependencies required for Tauri/WebKit compilation.
- Removed long-running soak tests and physical serial hardware tests from Codex-owned blocking work in this plan.
- Kept long soak, physical serial, and real-machine native desktop checks visible as user-owned release validation outside this Codex execution plan.

Validation:

- Ran desktop UI production build.
- Ran app-level tests with a separate local target directory to avoid the active Windows executable lock.
- Ran full workspace tests with `cargo test --workspace --locked`.
- Ran full workspace clippy with `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Ran `cargo fmt --all -- --check`.
- Ran editor package/schema contract tests.

### 2026-07-09 Headless Serve Reload Control Coverage

Completed:

- Added an integration test for the real long-lived `serve` loop.
- The test starts `baudbound serve` with webhook hosting enabled and no installed scripts.
- It imports a webhook script while the serve process is already running.
- It verifies that the trigger reload signal causes the running service to activate the new webhook listener.
- This original coverage used transitional JSON status/control files. It was replaced on 2026-07-10 by SQLite status assertions and authenticated IPC shutdown coverage.

Validation:

- Ran the new focused integration test.
- Ran `cargo test -p baudbound`.
- Ran `cargo clippy -p baudbound --all-targets --locked -- -D warnings`.
- Ran `cargo fmt`.

### 2026-07-09 SQLite Runner State Backend

Completed:

- Added the long-term architecture decision to this plan: SQLite for durable runner state, IPC for live runner control, and Tauri commands for the desktop UI bridge.
- Established SQLite as the only durable runner metadata store and IPC as the live control channel.
- Added `rusqlite` with bundled SQLite to avoid depending on system SQLite libraries.
- Added a `SqliteRunnerStore` backend under `baudbound-storage`.
- Added versioned schema initialization with `PRAGMA user_version`.
- Added schema tables for installed scripts, approvals, run records, service status, and durable runner signals.
- Enabled foreign keys, WAL journal mode, and a busy timeout for production-friendly multi-process behavior.
- Implemented the shared `ScriptStore` contract for the SQLite backend.
- Added SQLite-backed installed script import/update/list/find/remove/enable flows.
- Added SQLite-backed package hash verification while keeping `.bbs` package files as controlled package files on disk.
- Added SQLite-backed approvals.
- Added SQLite-backed run records.
- Added tested SQLite service-status round trips.
- Added tested one-shot SQLite trigger-reload signal behavior.
- Added a SQLite script lifecycle test covering import, approval, run records, hash verification, enable/disable, removal, and reload signaling.

Completed in the subsequent 2026-07-10 activation batch:

- Switched CLI, serve mode, desktop UI, and background runner to `SqliteRunnerStore`.
- Removed the unpublished filesystem/JSON storage backend so tests and production use the same SQLite implementation.
- Replaced JSON live control with authenticated loopback IPC.
- Wired desktop reload commands and headless serve control to IPC.

Validation:

- Ran `cargo test -p baudbound-storage --locked`.
- Ran `cargo test --workspace --locked`.
- Ran `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Ran `cargo clippy -p baudbound-storage --all-targets --locked -- -D warnings`.
- Ran `cargo test --workspace --locked`.
- Ran `cargo clippy --workspace --all-targets --locked -- -D warnings`.

### 2026-07-10 SQLite and IPC Activation

Completed:

- Switched every runner command, trigger service, status path, and desktop background-runner path to `SqliteRunnerStore`.
- Added automatic creation of `runner.sqlite3` in the runner home.
- Removed the unpublished filesystem/JSON metadata backend and migration code.
- Converted core and storage tests to use SQLite directly.
- Removed the obsolete `.service-control.json` implementation and its storage tests.
- Added authenticated loopback IPC with an operating-system-generated 256-bit token, bounded messages, read/write timeouts, protocol versioning, and loopback endpoint validation.
- Added live IPC reload and stop handling to the serve loop.
- Wired desktop trigger reload through IPC while keeping SQLite reload signals for durable script lifecycle changes.
- Stored service status snapshots in SQLite and redacted IPC credentials from CLI and desktop UI payloads.
- Restricted the SQLite database file to owner read/write permissions on Linux.
- Added IPC authentication/rejection tests and converted the long-lived CLI lifecycle test to SQLite status plus IPC shutdown.
- Made SQLite run-history ordering deterministic when multiple records share the same second-resolution timestamp.

Remaining architecture follow-up:

- Consider platform-specific named pipes/Unix sockets only as a later hardening change if authenticated loopback IPC proves insufficient.

Validation:

- Ran `cargo test -p baudbound-storage --locked`.
- Ran the SQLite/IPC CLI lifecycle integration suite.
- Ran `cargo test --workspace --locked`.
- Ran `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Ran `cargo fmt --all -- --check`.

### 2026-07-10 Runtime and Text Contract Hardening

Completed:

- Aligned For Each execution with the editor contract: only lists are accepted, including JSON-list strings and nested variable references; arbitrary objects and comma-separated text are rejected.
- Added complete condition-operator coverage, inverted-condition coverage, switch no-match behavior, false-first While behavior, and nested/empty/invalid For Each coverage.
- Made Variable Operation resolve templates consistently for set, increment, append-list, and object-field values.
- Added JSON container coercion for list/object and structured variable values exported as strings.
- Added editor-compatible nested object paths with numeric list indexes and strict path validation.
- Added all derived variable metadata fields: `$length`, `$count`, `$type`, and `$is_empty`.
- Matched JavaScript UTF-16 string length semantics for derived string metadata.
- Reserved every derived metadata name and aligned runtime variable-name validation with the editor identifier contract.
- Aligned clear defaults for object, list, duration, datetime, HTTP response, primitive, and file-path values.
- Rejected non-finite increment values instead of silently applying a fallback.
- Added a focused Variable Operation matrix covering every runtime-scope operation, defaults, metadata, and failure behavior.
- Added an operation-by-operation Text Transform matrix for all 17 editor options.
- Replaced permissive URL decoding with strict `decodeURIComponent`-compatible validation and aligned URL encoding with `encodeURIComponent` allowed characters.
- Fixed JSON unescape output for null and other non-string JSON values.
- Kept the new runtime and text suites in focused test modules instead of growing the existing broad test files.

Remaining runtime/data decisions:

- Define cooperative cancellation behavior for delays and long-running graph execution.

Validation:

- Ran `cargo test -p baudbound-runtime --locked` (30 tests passed).
- Ran `cargo test -p baudbound-actions --locked` (28 tests passed).
- Ran `cargo test --workspace --locked`.
- Ran `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Ran `cargo fmt --all -- --check`.
- Verified generated editor schemas are current.
- Ran all 34 editor contract tests.
- Ran desktop UI typecheck and production build.

### 2026-07-10 Concurrent Webhook Runtime Hardening

Completed:

- Replaced synchronous webhook execution in the serve loop with a fixed worker pool and bounded queue derived from available parallelism.
- Kept heartbeat, authenticated IPC stop/reload, route acceptance, and unrelated webhook requests responsive while workflows execute.
- Implemented the editor-exported `responseTimeoutSeconds` contract with strict positive finite validation and sub-second precision.
- Made immediate mode return its configured response without waiting for execution.
- Made waiting mode return Webhook Response node output when available, or the configured fallback at the response deadline.
- Defined timeout behavior explicitly: the client receives the fallback while workflow execution continues and is recorded normally.
- Added explicit 503 overload/stopping responses and non-fatal handling for clients that disconnect before a response is written.
- Preserved the listener, worker pool, pending responses, and active executions while trigger reload replaces webhook routes.
- Split webhook execution, HTTP translation, coordinator behavior, and their tests into focused modules.
- Added real loopback HTTP tests for immediate and waiting responses, custom response headers, timeout fallback, body-size rejection, method mismatch, and route reload during an active request.
- Added executor tests for parallel execution, bounded queue rejection, and dispatch failure propagation.

Remaining trigger hardening:

- WebSocket connection/message/disconnect/write concurrency coverage completed 2026-07-10.
- File Watch rapid-change and registration-replacement coverage completed 2026-07-10.
- Process-start polling/reload concurrency coverage completed 2026-07-10.

Validation:

- Ran `cargo test --workspace --locked` (220 tests passed).
- Ran `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Ran `cargo fmt --all -- --check`.
- Ran editor lint, typecheck, schema freshness, all 36 editor contract tests, and the production build.
- Ran desktop UI typecheck and production build.

### 2026-07-10 WebSocket Lifecycle and Concurrent Trigger Execution

Completed:

- Fixed trigger reload so WebSocket registration changes reuse the bound listener instead of attempting a second bind and failing with an address-in-use error.
- Added atomic route replacement, duplicate-path rejection, unchanged-route connection preservation, and deterministic disconnects for changed or removed routes.
- Replaced timestamp-based connection identifiers with collision-free registry sequence identifiers.
- Added native socket shutdown handles so service stop and route replacement unblock readers and remove stale connection IDs.
- Added bounded handshake/read/write timeouts and explicitly reset accepted sockets to blocking mode to prevent intermittent Windows handshake interruption.
- Enforced `max_message_bytes` inside Tungstenite before oversized frames or fragmented messages can be fully accumulated.
- Added configurable `websockets.max_connections`, CLI override validation, desktop simple-mode configuration, diagnostics, template defaults, and documentation.
- Added a bounded connection permit system with an HTTP 503 handshake response when capacity is exhausted.
- Replaced the WebSocket service monolith with focused connection, listener, registry, route, service, and test modules.
- Extracted webhook execution into a shared bounded trigger executor and routed WebSocket, file-watch, process-start, and serial listener workflows through the same concurrent execution boundary.
- Replaced the unbounded listener event channel with bounded backpressure and explicit persisted/logged failures when the execution queue is saturated.
- Added real loopback tests for concurrent clients, text/JSON/binary payloads, response writes, headers/query data, connection-ID uniqueness, 404 routes, 503 capacity, protocol message limits, disconnect cleanup, shutdown, and route reload.
- Stress-ran the WebSocket lifecycle suite repeatedly and fixed the Windows accepted-socket nonblocking inheritance race it exposed.

Remaining trigger hardening:

- File Watch create/modify/delete/rename, recursion, burst, and registration-replacement coverage completed 2026-07-10.
- Process-start polling/reload concurrency coverage completed 2026-07-10.
- Schedule reload, drift, and missed-interval coverage completed 2026-07-10.

Validation:

- Ran `cargo test --workspace --locked` (228 tests passed).
- Ran `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Ran `cargo fmt --all -- --check`.
- Ran editor lint, typecheck, schema freshness, all 36 editor contract tests, and the production build.
- Ran desktop UI typecheck and production build.

### 2026-07-10 File Watch Lifecycle and Reload Hardening

Completed:

- Added an explicit `recursive` File Watch option to the editor, generated node schema, and runner contract, defaulting to non-recursive behavior.
- Made File Watch paths static subscription configuration and reject runtime variable templates in both editor verification and the runner parser.
- Changed file-target subscriptions to watch the parent directory and filter the configured file so delete, recreate, and rename events remain observable.
- Normalized platform events to `created`, `modified`, `deleted`, and `renamed`, ignored access-only noise, filtered unrelated paths, and emitted the rename destination.
- Adopted the stable `notify-debouncer-full` companion crate to stitch native Windows rename pairs and remove duplicate filesystem event noise without a custom timing state machine.
- Dropped the old File Watch service before constructing replacement registrations during trigger reload, preventing old and new watchers from overlapping.
- Split File Watch parsing, target selection/service ownership, event normalization, and tests into focused modules.
- Added real temporary-filesystem tests for modify/delete/recreate/rename, recursive and non-recursive directories, 24-file bursts, missing targets, static-path enforcement, and watcher replacement.
- Stress-ran the complete File Watch suite ten consecutive times on Windows.
- Added an editor contract test that protects the static path and recursive schema shape.

Remaining trigger hardening:

- Process-start polling/reload concurrency coverage completed 2026-07-10.
- Schedule reload, drift, and missed-interval coverage completed 2026-07-10.
- Cross-service trigger reload boundary coverage completed 2026-07-10.

Validation:

- Ran `cargo test --workspace --locked` (235 tests passed).
- Ran `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Ran `cargo fmt --all -- --check`.
- Ran editor lint, typecheck, schema freshness, all 37 editor contract tests, and the production build.
- Ran desktop UI typecheck and production build.

### 2026-07-10 Process-Start and Schedule Lifecycle Hardening

Completed:

- Replaced process-start restart-on-reload behavior with one persistent, named polling worker controlled through a bounded command channel.
- Added synchronous initialization and reload acknowledgements so the runner has explicit watcher lifecycle boundaries.
- Made shutdown interrupt the worker immediately instead of waiting through the one-second polling sleep.
- Identified processes by process ID plus process start time, preventing duplicate events while recognizing PID reuse.
- Preserved unchanged window-title candidates across reload and baselined newly added or changed registrations so already-running processes do not fire retroactively.
- Defined the reload boundary so processes first observed during reload notify only unchanged registrations; changed, removed, and newly added registrations cannot emit stale events.
- Added duplicate registration rejection and corrected process watcher diagnostics to report registration count rather than thread count.
- Split process-start service ownership, worker control, matching engine, snapshots, state tracking, event creation, parsing, native Windows lookup, and tests into focused modules.
- Added deterministic state/engine tests plus a real child-process test covering event delivery, duplicate suppression, in-place reload, stale-registration rejection, and prompt shutdown.
- Stress-ran the real process-start lifecycle suite ten consecutive times on Windows.
- Added positive fractional schedule intervals matching the editor contract, with strict finite and one-nanosecond bounds on both editor and runner sides.
- Preserved unchanged schedule deadlines across trigger reload while resetting changed and newly added schedules from the reload time.
- Defined missed schedule behavior as one coalesced event with `missed_intervals`, then advanced in constant time to the next point on the original cadence.
- Replaced schedule task vectors with deterministic keyed ownership and rejected duplicate registrations.
- Split schedule parsing, service state, and timing tests into focused modules.
- Added editor contract coverage for static schedule duration validation.

Remaining trigger work:

- Complete final serial and hotkey platform verification as part of their dedicated readiness areas.

Validation:

- Ran `cargo test --workspace --locked` (243 tests passed).
- Ran `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Ran `cargo fmt --all -- --check`.
- Ran editor lint, typecheck, schema freshness, all 38 editor contract tests, and the production build.
- Ran desktop UI typecheck and production build.

### 2026-07-10 Capability, Process, and Security Hardening

Completed:

- Added an editor-generated, versioned node capability contract sourced directly from node definitions and embedded into the Rust security crate at compile time.
- Added a contract test that requires every editor node to appear exactly once in the generated runner capability map and makes stale generated output fail CI.
- Recalculated required capabilities from the executable graph and required `capabilities.json` to match exactly during package validation, import, execution, and trigger registration.
- Rejected missing, extra, and duplicate capabilities, duplicate permissions, unknown executable action types, and malformed programs.
- Added an end-to-end import test proving that a package cannot hide an executable node by omitting its capability declaration.
- Added abuse-case coverage proving that shell, process-kill, delete, webhook, and WebSocket declarations cannot bypass their risk or independent policy gates.
- Hardened process lookup so name/path matches select a deterministic lowest PID and Linux executable paths remain case-sensitive while Windows paths remain case-insensitive.
- Fixed process argument parsing so quoted arguments work without corrupting ordinary Windows backslash paths.
- Added focused process and shell tests for arguments, working directories, stdout/stderr, nonzero exits, missing executables, PID/name/path status, invalid modes, process termination, and policy classifications.
- Corrected app integration fixtures so exported test packages declare the capabilities their executable graphs actually require.

Remaining process/security work:

- Verify the desktop approval modal end to end, including approval invalidation after package updates.

Validation:

- Ran `cargo test --workspace --locked` (202 tests passed).
- Ran `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Ran all 35 editor contract tests and verified generated schemas/contracts are current.
- Ran desktop UI typecheck and production build.

### 2026-07-10 Native Process Matching and Sub-script Boundaries

Completed:

- Added native Win32 window-title lookup for Process Status and Kill Process without shell or PowerShell execution.
- Routed only the window-title process mode through the desktop adapter; PID, process-name, and executable-path modes remain in the shared cross-platform backend.
- Made Windows window-title process selection deterministic by selecting the lowest matching process ID.
- Added native Win32 process termination through `OpenProcess` and `TerminateProcess`.
- Added config-sensitive editor verification and runner import rules so Process Status, Kill Process, and App / Process Started allow window-title matching only for an explicit Windows Desktop target.
- Added an editor contract gate requiring all three node definitions to retain the config-sensitive Windows rule.
- Implemented native Windows window-title matching for App / Process Started.
- Added a per-registration process tracker that ignores processes already running when the service starts, tracks each new PID until its window appears, suppresses duplicate events, removes exited candidates, and handles PID reuse.
- Preserved case-sensitive executable-path matching on Linux while retaining Windows case-insensitive behavior.
- Added Sub-script coverage for missing targets, failed parent run persistence, independent child approval enforcement, and persisted child run ID linkage.
- Split the Win32 process action backend, process-start state tracker, native trigger lookup, and Sub-script tests into focused modules.

Remaining related work:

- Perform user-owned real-window validation for successful Windows title query/termination and process-start title events.
- Process-start polling/reload concurrency coverage completed 2026-07-10.
- Verify the desktop approval modal end to end.

Validation:

- Ran `cargo test --workspace --locked` (211 tests passed).
- Ran `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Ran `cargo fmt --all -- --check`.
- Ran editor lint, typecheck, schema freshness, and all 36 editor contract tests.
- Ran desktop UI typecheck and production build.

### 2026-07-10 Calculation, HTTP, and Filesystem Hardening

Completed:

- Added a focused Calculate matrix covering all documented arithmetic operators, precedence, grouping, unary operators, exponents, scientific notation, round/floor/ceil, min/max, and every random function shape.
- Aligned negative half-value rounding with JavaScript `Math.round()` instead of Rust's away-from-zero tie behavior.
- Replaced timestamp-derived pseudo-random values with operating-system randomness and a 53-bit unit-interval conversion.
- Added malformed-expression coverage for empty input, division/modulo by zero, non-finite exponents/numbers, invalid function arity, unknown functions, missing parentheses, trailing tokens, and invalid characters.
- Added an HTTP matrix covering all seven editor methods and the GET/HEAD body policy.
- Added HTTP object/list header, user-agent, request body, parsed JSON, raw body, response header, status, duration, connection failure, invalid configuration, timeout, and 2 MiB response coverage.
- Added Download File coverage for successful writes, non-success HTTP statuses, destination creation, overwrite protection, and explicit overwrite.
- Added Read/Write/Copy/Move/Delete failure and boundary coverage using isolated temporary directories.
- Prevented Copy File and Move File from targeting the same resolved file, including equivalent path spellings, so overwrite cannot truncate or remove the source.
- Required Copy File and Move File sources to be regular files.
- Replaced the Windows move overwrite sequence that deleted the destination first with native `MoveFileExW` replace/cross-volume behavior.
- Kept Linux Move File on the native atomic rename path.
- Split calculation, HTTP, filesystem, and native move code/tests into focused modules.

Validation:

- Ran `cargo test -p baudbound-runtime -p baudbound-actions --locked` (33 runtime and 40 action tests passed).
- Ran `cargo test --workspace --locked`.
- Ran `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Verified generated editor schemas are current.
- Ran all 34 editor contract tests.
- Ran desktop UI typecheck and production build.

### 2026-07-11 Durable Variables and Encrypted Secrets

Completed:

- Defined `runtime`, per-script `persistent`, and runner-wide `global` as the only writable variable scopes.
- Added SQLite-backed persistent/global values with versioned compare-and-set writes so concurrent executions do not silently lose updates.
- Defined secrets as runner-managed read-only inputs declared by packages rather than writable variables.
- Added strict manifest secret declaration validation, generated schema coverage, and configuration-derived `read_secret`, persistent-write, and global-write permissions/capabilities.
- Added authenticated XChaCha20-Poly1305 encryption for SQLite secret values with script/name associated data and operating-system randomness.
- Added desktop key ownership through the operating-system credential vault and explicit headless key ownership through `BAUDBOUND_SECRET_KEY`, with no plaintext fallback.
- Added hidden CLI secret entry and desktop Security-tab management that expose only configured/missing status, never stored values.
- Loaded typed required/optional secrets into runtime execution and excluded secret names and values from persisted reports.
- Added recursive report/log/error redaction and editor simulation redaction for direct or copied secret values.
- Added editor secret declaration management and explicit session-only simulation values that are reset on import and never exported.
- Prevented derived metadata generation for secrets and rejected the obsolete writable `secret` Variable Operation scope.
- Added focused package, storage encryption/CAS, runtime state/redaction, security derivation, core, CLI-parser, and editor contract coverage.

Remaining related work:

- Define cooperative cancellation behavior for long-running graph execution.
- Complete the existing desktop approval reliability and release packaging work.

Validation:

- Ran `cargo fmt --all -- --check`.
- Ran `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Ran `cargo test --workspace --locked` (254 tests passed).
- Built the `baudbound` application with the locked dependency graph.
- Ran editor lint, typecheck, generated-schema freshness, and all 39 editor contract tests.
- Built the editor production bundle.
- Ran all 26 editor Chromium/Firefox end-to-end tests.
- Typechecked and built the desktop React UI production bundle.

## Tracking Notes

Update this document after each runner development batch:

- Move completed items into a completed section or check them off.
- Update readiness percentages only when validated by tests or manual verification.
- Add exact platform limitations as soon as they are discovered.
- Do not mark a feature production-ready merely because it compiles.

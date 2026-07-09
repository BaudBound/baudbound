# Runner Production Readiness Plan

Snapshot date: 2026-07-09

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
- Future config migration metadata.

IPC owns live control:

- Stop running service/background runner.
- Reload trigger registrations.
- Query live runner status when a runner process is active.
- Future live commands that should not be represented as files.

Tauri commands own desktop UI communication:

- React UI must call Tauri commands for runner operations.
- Tauri commands may read/write SQLite and communicate with the live runner through IPC.
- The desktop UI must not depend on service-control JSON files as a production control channel.

Files remain valid only for file-shaped data:

- `.bbs` packages and package assets.
- User-editable TOML config.
- Optional exported diagnostics/log bundles.
- Human-authored deployment templates and documentation.

The existing `index.json`, `approvals.json`, `runs.jsonl`, `service-status.json`, `.service-control.json`, and `.trigger-reload` storage/control files are transitional compatibility paths. They must either be migrated into SQLite/IPC before first release or explicitly removed before release. The final production runner must not rely on JSON files for live process control.

## Current Readiness Snapshot

| Area | Current estimate | Notes |
| --- | ---: | --- |
| Runtime execution engine | 88% | Core graph execution, variables, conditions, loops, while, for-each, switch, calculate, delay, log, and external action dispatch exist. Needs broader end-to-end test coverage. |
| Action coverage | 83% | Most editor actions have runner handlers. Editor-to-runner support ownership is now CI-gated. Native desktop behavior still needs real-machine verification and platform gating review. |
| Trigger services | 83% | Schedule, file watch, webhook, websocket, serial, startup, process started, and hotkey services exist. Long-lived serve reload/stop behavior now has automated CLI coverage; long soak validation is user-owned for now. |
| Serial/device system | 80% | Logical device IDs, reconnect, USB identity validation, port rebinding, scanner, and UI exist. Codex-owned work focuses on config/import/UI correctness; physical multi-device validation is user-owned for now. |
| Security/approval/storage | 86% | Import validation, package hash checks, minimum runner version enforcement, approvals, stale approval states, and run records exist. SQLite backend now implements the script lifecycle contract, but the app still needs to switch active storage from JSON/index files before release. |
| Desktop UI | 75% | Main tabs and workflows exist. Needs final polish, error states, packaging validation, and production install behavior. |
| Codebase maintainability | 92% | Inline tests were moved out and action/runtime/trigger/core/storage domain modules were extracted. Trigger services live under `services/`, action implementations live under `actions/`, runtime graph/config/control/condition/template/variable/calculation helpers live under `runtime/`, core package/run-record/serial/status/sub-script/trigger/version logic lives in focused modules, and storage filesystem/metadata/service-control/approval/run helpers live under `storage/`. Remaining maintainability work is mostly fixture cleanup and targeted splits only where production files continue to grow. |
| Packaging/release | 40% | Initial runner CI quality gate exists. Still needs Tauri release packaging, GitHub Releases updater artifacts, Linux AppImage, versioning, signing decisions, and documentation. |
| Cross-platform native support | 50% | First release supports Windows and Linux only. Windows has the strongest native desktop path; Linux desktop support must be verified or precisely gated. |
| Overall first production runner readiness | 80% | Good foundation, but not safe to publish as production until the SQLite/IPC architecture migration and the checklist below are complete. |

## Feature Coverage Baseline

The runner must support or intentionally reject every feature exported by the editor.

### Control Flow

| Editor node | Runner status | Production requirement |
| --- | --- | --- |
| If / Else | Implemented | Add more condition matrix tests, including inverted conditions. |
| Switch | Implemented | Test default/no-match behavior and all exported case shapes. |
| Loop | Implemented | Verify loop branch does not need to return to loop input. |
| While | Implemented | Add long-running and false-first-condition tests. |
| For Each | Implemented | Test lists from variables, nested object paths, empty lists, and non-list rejection. |

### Runtime/Data Actions

| Editor node | Runner status | Production requirement |
| --- | --- | --- |
| Variable Operation | Implemented | Test all operations and derived metadata for strings, lists, and objects. |
| Calculate | Implemented | Test supported operators/functions against editor docs. |
| Format Text | Implemented | Add operation-by-operation tests matching editor options. |
| Log | Implemented | Ensure logs appear consistently in CLI and desktop UI. |
| Delay | Implemented | Add cancellation/stop behavior decision for long delays. |

### File, Network, Process, and System Actions

| Editor node | Runner status | Production requirement |
| --- | --- | --- |
| HTTP Request | Implemented | Add timeout, headers, body, failure, and large response tests. |
| Download File | Implemented | Add overwrite/path/error tests. |
| Read File | Implemented | Confirm encoding and permissions behavior. |
| Write File | Implemented | Confirm append/overwrite and permissions behavior. |
| Delete File | Implemented | Verify path validation and dangerous approval flow. |
| Copy File | Implemented | Verify overwrite behavior and path errors. |
| Move File | Implemented | Verify overwrite behavior and path errors. |
| Run Process | Implemented | Test wait/no-wait, args, exit code, stdout/stderr. |
| Process Status | Implemented | Test by name/path/window title/PID where supported. |
| Kill Process | Implemented | Test PID mode and name/path modes safely. |
| Shell Command | Implemented | Keep dangerous; verify approval gating and output capture. |
| Sub-script | Implemented | Test recursion prevention, missing script, approval, and run record linkage. |

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
| Schedule | Implemented | Add automated coverage for reload, clock drift, and missed interval behavior where practical. Long soak validation is user-owned for now. |
| File Watch | Implemented | Test create/modify/delete/rename and recursive behavior. |
| Webhook | Implemented | Test wait-for-response mode, timeout, body limits, headers, methods, and concurrent requests. |
| WebSocket | Implemented | Test connection lifecycle, message payloads, max message size, and writes back to connection. |
| Serial Input | Implemented | Add automated coverage for config, reconnect decisions, auto-rebind decisions, identity matching, and line/raw modes where practical. Physical multi-device validation is user-owned for now. |
| Hotkey | Implemented | Verify native hotkey support and OS permission requirements. |
| Startup | Implemented as runner-start trigger | Document that headless startup is controlled by user service manager; desktop app startup behavior still needs release decision. |
| App/Process Started | Implemented | Test polling accuracy, duplicate suppression, process matching modes. |

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

- Webhook concurrent requests.
- Webhook wait-for-response success, timeout, and missing response behavior.
- WebSocket connect/message/disconnect and write response.
- File watch with rapid changes.
- Process-started duplicate suppression.
- Trigger reload while events are arriving.

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

- Installed script metadata, approvals, run records, and package hashes are migrated to SQLite.
- Package hash mismatch blocks execution.
- Updated packages invalidate approval.
- Dangerous/high/medium permissions are visible and require approval where intended.
- Permissions are recalculated by runner and compared to package declarations.
- Capabilities are recalculated by runner and compared to package declarations.
- Minimum runner version is enforced.
- Tampered `.bbs` packages fail import or execution safely.
- Sub-scripts cannot bypass approval.
- Shell and process-kill permissions are clearly high-risk/dangerous.

Done when:

- A small abuse-case suite exists.
- Desktop UI approval modal works reliably.
- CLI approval commands remain complete for headless users.

### 7. Desktop UI Release Polish

Required:

- Scripts tab: import/update/remove/run/approve/revoke flows are clear.
- Runs tab: logs and variables are readable for failed and successful runs.
- Config tab: simple and advanced modes both work, with TOML validation.
- Devices tab: scanner and add-to-config flow work reliably.
- Security tab: approval/risk/package hash language is user-friendly.
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
- SQLite migration tests.
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

### 12. SQLite and IPC Production Migration

Required:

- Move installed script metadata from `index.json` into SQLite.
- Move approvals from `approvals.json` into SQLite.
- Move run history from `runs.jsonl` into SQLite.
- Move service status from `service-status.json` into SQLite.
- Replace `.service-control.json` live stop/reload requests with a real IPC channel.
- Decide whether `.trigger-reload` becomes an IPC request, a SQLite durable signal, or both depending on whether the serve process is currently running.
- Add one-time migration from existing JSON files where practical, or document that pre-release storage is not migrated if this happens before any public runner release.
- Keep `.bbs` packages as files under a controlled packages directory and store only metadata/hash/path references in SQLite.
- Keep TOML config as the user-editable config source unless a future release intentionally moves config into the database.
- Add tests for schema initialization, idempotent migrations, state round-trips, and IPC stop/reload behavior.

Done when:

- New runner homes create a SQLite database automatically.
- CLI, serve mode, desktop UI, and tests use SQLite-backed durable state.
- Live stop/reload control no longer uses JSON files.
- Existing JSON control/state code is removed or isolated behind a clearly named pre-release migration path.

## Missing or Weak Areas To Investigate

These are not confirmed blockers yet, but must be reviewed before release:

- Whether terminal bell is acceptable for Beep, or whether native OS beep APIs are required.
- Whether Open Application uses acceptable native behavior on each supported desktop platform.
- Whether process/window title matching is available outside desktop contexts and gated correctly.
- Whether Linux desktop native input/notification behavior works under Wayland, X11, or both.
- Whether package/run logs need retention limits or cleanup controls.
- Whether secrets and persistent variables are fully implemented or should be documented as not in first release.
- Whether minimum runner version should be bumped before first release.
- Whether runner config migration is needed for future changes.
- Whether old packages using removed target runtimes should be rejected with a clear unsupported-platform message.
- Exact IPC transport choice for first release: local TCP loopback, named pipe/Unix domain socket, or a small cross-platform IPC crate.

## First Release Exit Criteria

The first production runner release can be published only when all of these are true:

- [ ] Every editor node/trigger is implemented or explicitly blocked per platform.
- [ ] Editor verification and runner import agree on platform compatibility.
- [ ] All required automated test suites pass in CI.
- [ ] Native desktop action support is either implemented and automated where possible, or gated with clear unsupported-platform errors.
- [ ] Headless serve mode passes automated command/reload/status checks.
- [ ] Serial device config and UI flows are verified automatically where possible.
- [ ] Webhook/WebSocket trigger tests pass under concurrency.
- [ ] Package hash and approval flows are verified in CLI and desktop UI.
- [ ] Desktop UI can complete import, approve, run, view logs, edit config, scan devices, and manage background runner state.
- [ ] Durable state uses SQLite.
- [ ] Live runner control uses IPC instead of JSON control files.
- [ ] Release packages can be built from a clean checkout.
- [ ] First-release documentation exists.

User-owned release validation outside this Codex execution plan:

- Real-machine native desktop action checks.
- Long-running soak tests.
- Physical serial hardware tests, including multi-device reconnect/rebind behavior.

## Suggested Execution Order

1. Build the editor-to-runner feature matrix and use it to find exact gaps.
2. Close missing implementation gaps or add precise platform rejections.
3. Migrate durable runner state to SQLite.
4. Replace live service-control JSON files with IPC.
5. Add automated tests for graph execution, actions, triggers, package validation, storage, and IPC.
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
- Extracted `baudbound-storage` service status and service-control request handling into `baudbound-storage/src/storage/service_control.rs`.
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
- It writes a targeted `.service-control.json` stop request and verifies the serve process exits cleanly.
- The test reads `service-status.json` instead of using internal test-only hooks, so it covers the same status/control files used by headless deployments and the desktop UI.

Validation:

- Ran the new focused integration test.
- Ran `cargo test -p baudbound`.
- Ran `cargo clippy -p baudbound --all-targets --locked -- -D warnings`.
- Ran `cargo fmt`.

### 2026-07-09 SQLite Runner State Backend

Completed:

- Added the long-term architecture decision to this plan: SQLite for durable runner state, IPC for live runner control, and Tauri commands for the desktop UI bridge.
- Marked JSON service status/control files as transitional pre-release compatibility paths instead of production architecture.
- Added `rusqlite` with bundled SQLite to avoid depending on system SQLite libraries.
- Added a `SqliteRunnerStore` backend under `baudbound-storage`.
- Added versioned schema initialization with `PRAGMA user_version`.
- Added schema tables for installed scripts, approvals, run records, service status, durable runner signals, and migration metadata.
- Enabled foreign keys, WAL journal mode, and a busy timeout for production-friendly multi-process behavior.
- Implemented the shared `ScriptStore` contract for the SQLite backend.
- Added SQLite-backed installed script import/update/list/find/remove/enable flows.
- Added SQLite-backed package hash verification while keeping `.bbs` package files as controlled package files on disk.
- Added SQLite-backed approvals.
- Added SQLite-backed run records.
- Added tested SQLite service-status round trips.
- Added tested one-shot SQLite trigger-reload signal behavior.
- Added a SQLite script lifecycle test covering import, approval, run records, hash verification, enable/disable, removal, and reload signaling.

Still remaining:

- Switch the active app store from `FilesystemScriptStore` to `SqliteRunnerStore`, or add a clean store selector while migration is in progress.
- Add a one-time pre-release migration path from JSON/index storage to SQLite if existing local test installs must be preserved.
- Replace JSON service-control files with IPC stop/reload commands.
- Wire desktop Tauri commands and headless serve control to the final IPC channel.

Validation:

- Ran `cargo test -p baudbound-storage --locked`.
- Ran `cargo clippy -p baudbound-storage --all-targets --locked -- -D warnings`.
- Ran `cargo test --workspace --locked`.
- Ran `cargo clippy --workspace --all-targets --locked -- -D warnings`.

## Tracking Notes

Update this document after each runner development batch:

- Move completed items into a completed section or check them off.
- Update readiness percentages only when validated by tests or manual verification.
- Add exact platform limitations as soon as they are discovered.
- Do not mark a feature production-ready merely because it compiles.

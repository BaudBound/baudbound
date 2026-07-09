# Runner Production Readiness Plan

Snapshot date: 2026-07-09

This document tracks what must be completed before publishing the first production BaudBound runner release. The editor is considered mostly ready feature-wise, so this plan focuses on making the Rust runner support every editor-exported feature, or explicitly reject unsupported platform combinations before execution.

## Release Goal

Publish the first production runner release when:

- Every editor node has a corresponding runner implementation, validation path, and platform support rule.
- Every unsupported platform/action combination is blocked in both the editor and runner.
- Runner import, approval, storage, trigger, and execution behavior is tested under realistic conditions.
- The desktop runner can be installed and used by normal users.
- The headless runner can be used manually with clear deployment instructions.
- No feature is half-implemented, silently ignored, or allowed to fail only at runtime when it could be rejected earlier.

## Current Readiness Snapshot

| Area | Current estimate | Notes |
| --- | ---: | --- |
| Runtime execution engine | 88% | Core graph execution, variables, conditions, loops, while, for-each, switch, calculate, delay, log, and external action dispatch exist. Needs broader end-to-end test coverage. |
| Action coverage | 82% | Most editor actions have runner handlers. Native desktop behavior still needs real-machine verification and platform gating review. |
| Trigger services | 80% | Schedule, file watch, webhook, websocket, serial, startup, process started, and hotkey services exist. Needs soak testing and reload/race validation. |
| Serial/device system | 78% | Logical device IDs, reconnect, USB identity validation, port rebinding, scanner, and UI exist. Needs physical multi-device testing. |
| Security/approval/storage | 86% | Import validation, package hash checks, approvals, stale approval states, and run records exist. Needs final abuse-case and UX audit. |
| Desktop UI | 75% | Main tabs and workflows exist. Needs final polish, error states, packaging validation, and production install behavior. |
| Packaging/release | 35% | Not release-ready. Needs Tauri packaging, GitHub Releases updater artifacts, Linux AppImage, release CI, versioning, signing decisions, and documentation. |
| Cross-platform native support | 50% | First release supports Windows and Linux only. Windows has the strongest native desktop path; Linux desktop support must be verified or precisely gated. |
| Overall first production runner readiness | 72% | Good foundation, but not safe to publish as production until the checklist below is complete. |

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
| Schedule | Implemented | Soak test timers, reload, clock drift, and missed interval behavior. |
| File Watch | Implemented | Test create/modify/delete/rename and recursive behavior. |
| Webhook | Implemented | Test wait-for-response mode, timeout, body limits, headers, methods, and concurrent requests. |
| WebSocket | Implemented | Test connection lifecycle, message payloads, max message size, and writes back to connection. |
| Serial Input | Implemented | Test reconnect, auto-rebind, identity matching, line/raw modes, and multiple devices. |
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
- Service status/control files cannot get stuck in stale states.
- Long-running serve process can be stopped cleanly.
- Logs are readable and bounded enough to avoid unbounded growth issues.

Done when:

- Headless runner can run continuously through a soak test.
- Import/update/remove workflows are documented and verified.
- No desktop UI dependency exists for headless use.

### 4. Trigger Soak and Race Testing

Required tests:

- Schedule triggers for long-running processes.
- Webhook concurrent requests.
- Webhook wait-for-response success, timeout, and missing response behavior.
- WebSocket connect/message/disconnect and write response.
- Serial disconnect/reconnect/rebind with one and multiple devices.
- File watch with rapid changes.
- Process-started duplicate suppression.
- Trigger reload while events are arriving.

Done when:

- Trigger services survive reloads and concurrent events.
- Failed trigger events create understandable logs.
- A script update cannot leave old trigger registrations active.

### 5. Serial Device Production Validation

The serial system is important enough to require real hardware testing.

Required:

- Device ID in editor maps to runner TOML device config.
- Serial input and serial write use the same logical device ID.
- Auto reconnect works after physical disconnect/reconnect.
- USB vendor/product validation prevents wrong-device connection.
- Auto rebind updates config when COM ports change.
- Auto rebind handles two devices swapping ports.
- Serial number/manufacturer/product matching is documented.
- Devices tab scanner creates valid config entries.

Done when:

- Multi-device test passes.
- Wrong-device test fails safely.
- Config update behavior is visible and understandable in UI/logs.

### 6. Security and Approval Final Audit

Required:

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

Done when:

- CI is green on a clean checkout.
- CI covers the runner/editor contract so future editor nodes cannot be added without runner awareness.

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

## First Release Exit Criteria

The first production runner release can be published only when all of these are true:

- [ ] Every editor node/trigger is implemented or explicitly blocked per platform.
- [ ] Editor verification and runner import agree on platform compatibility.
- [ ] All required automated test suites pass in CI.
- [ ] Real Windows Desktop native action tests pass.
- [ ] Headless serve mode passes a soak test.
- [ ] Serial multi-device reconnect/rebind tests pass.
- [ ] Webhook/WebSocket trigger tests pass under concurrency.
- [ ] Package hash and approval flows are verified in CLI and desktop UI.
- [ ] Desktop UI can complete import, approve, run, view logs, edit config, scan devices, and manage background runner state.
- [ ] Release packages can be built from a clean checkout.
- [ ] First-release documentation exists.

## Suggested Execution Order

1. Build the editor-to-runner feature matrix and use it to find exact gaps.
2. Close missing implementation gaps or add precise platform rejections.
3. Add automated tests for graph execution, actions, triggers, package validation, and storage.
4. Run real native desktop action verification on Windows.
5. Run serial hardware validation.
6. Harden headless serve/reload workflows.
7. Finish desktop UI release polish.
8. Add release packaging and CI.
9. Write first-release docs.
10. Run final clean-machine release rehearsal.

## Tracking Notes

Update this document after each runner development batch:

- Move completed items into a completed section or check them off.
- Update readiness percentages only when validated by tests or manual verification.
- Add exact platform limitations as soon as they are discovered.
- Do not mark a feature production-ready merely because it compiles.

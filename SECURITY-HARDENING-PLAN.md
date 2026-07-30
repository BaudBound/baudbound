# BaudBound Security Hardening Plan

This file tracks the runner hardening work from the security audit.

## Implemented

- Renamed runner permissions to dot-style names such as `process.run`, `process.shell`, `http.request`, `network.webhook`, and `network.websocket`.
- Split webhook/WebSocket script capability from public listener exposure. Packages now declare trigger capability; public bind policy is enforced when listeners start.
- Added `security.policy.allow_private_http_requests = false` and enforced it for `action.http`, including private IP, localhost, DNS-to-private, and redirect target checks.
- Removed string process argument parsing. Process/application arguments now use arrays of strings only.
- Added sensitive-operation confirmation for script enable/disable and Script Setting mutations.
- Added bounded delete permission calculation with `file.delete.limited` and `file.delete.any`.
- Replaced limited-path check-then-open filesystem access with `cap-std` directory capabilities. Relative script paths are now opened, copied, moved, downloaded, and deleted through a workspace directory handle that prevents symlink escapes and path-rebinding attacks.
- Applied private-network egress policy to `action.file.download` as well as `action.http`. DNS results are validated once, pinned into the HTTP client, checked again on every redirect, and system proxies are disabled for script-originated requests.
- Expanded secret redaction to cover values nested inside structured secrets plus JSON-escaped, percent-encoded, form-encoded, Base64, URL-safe Base64, and hexadecimal representations.
- Aligned the runner's direct HTTP dependency with reqwest 0.13, removing the duplicate reqwest 0.12 dependency branch.
- Split secondary desktop views into lazy-loaded production chunks. The entry chunk is now about 315 kB instead of the previous roughly 1.03 MB main bundle, and Vite no longer reports an oversized chunk.
- Improved CLI lifecycle timeout failures with the elapsed timeout, runner database path, runner-home contents, and the last observed service status.
- Bound Script Setting confirmations to the submitted value by recomputing and verifying its SHA-256 digest in the backend after confirmation consumption.
- Synchronized the canonical, editor, and runner contracts. Permission names use dot notation while program action and trigger discriminators retain their runner identifiers.
- Added an editor/runner node-catalog parity test and aligned process/application argument arrays plus bounded read, write, and delete path classification.

## Verified

- `cargo check --target-dir target\hardening-check`
- `cargo fmt --all -- --check`
- `cargo test --workspace --target-dir target\hardening-check`
- `cargo clippy --workspace --all-targets --target-dir target\hardening-check -- -D warnings`
- `pnpm test`
- `pnpm build`
- `cargo deny check`
- `node scripts/validate.mjs` in the canonical contracts repository
- Editor `pnpm lint`, `pnpm typecheck`, `pnpm schemas:check`, `pnpm test`, `pnpm build`, and `pnpm e2e`

`cargo deny check` passes advisories, bans, licenses, and sources. It still reports duplicate-version warnings where third-party crates require incompatible major versions; no remaining duplicate can be removed solely by changing a direct BaudBound dependency without dropping functionality or replacing an upstream component.

## Status

All items from this hardening plan are implemented and verified. There are no open follow-up items in this plan.

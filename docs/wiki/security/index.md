---
title: Security Model
description: Understand BaudBound trust boundaries, package checks, approval, secrets, and operator responsibilities.
tags: [security, approvals, integrity]
---
# Security Model

An automation can read data, change files, start programs, or control input. BaudBound therefore treats every imported package as untrusted until the local runner has checked it and the user has approved the exact installed revision.

The editor describes what the package intends to do. The runner makes the final decision. It reads the package, checks the graph and settings, calculates required access from the nodes, checks platform support, verifies the installed file fingerprint, and requires approval before an action can affect the machine.

Approval means that you accept the reviewed operations for one exact package revision. It does not mean the package came from a trusted author. It also does not allow invalid or unsupported behavior.

## Threat model

The security model is designed to limit these failures:

- a package understates the files, processes, input devices, network listeners, or secrets it uses.
- a package is changed after an operator approved it.
- a script runs on a platform where its native behavior is unavailable.
- a listener is exposed more broadly than the operator intended.
- one script calls another script that has not been reviewed.
- secret plaintext is included in a package, log, or ordinary variable snapshot.
- a UI bug attempts an operation the Rust backend has not authorized.

BaudBound cannot decide whether an otherwise truthful automation is appropriate for your machine. An approved Shell node can still run a destructive command. An approved File Delete node can still delete the path its configuration resolves to. Review remains an operator responsibility.

## Trust boundaries

| Boundary | Trusted responsibility | Not trusted as authority |
| --- | --- | --- |
| Editor | Produces a structured graph, target, and review declarations | Its permission, capability, and risk claims |
| `.bbs` package | Portable input to strict parsing | Filename, display name, or manually edited archive contents |
| Runner parser | Enforces archive, document, graph, and configuration rules | Unknown nodes or extra package files |
| Security derivation | Recalculates permissions, capabilities, risk, and target compatibility | Package declarations that do not match the graph |
| Operator approval | Accepts one installed package revision and its access | Future updates or another script with a similar name |
| Native adapter | Performs an authorized OS operation | Shell emulation of an unsupported native feature |
| Desktop UI | Presents state and sends Tauri requests | Authorization decisions. Rust checks them again |
| Network edge | Delivers webhook or WebSocket input | Internet origin, proxy configuration, or payload safety by default |

## Validation before side effects

The runner performs the relevant checks before graph execution:

1. Open the archive and reject malformed, duplicate, unexpected, absolute, or traversal paths.
2. Parse required JSON documents and validate package relationships.
3. Validate graph IDs, edges, node configuration, variables, entry triggers, and supported language/version rules.
4. Derive permissions, capabilities, risk, and target support from the executable graph.
5. Reject declarations that do not exactly match those derived values.
6. Verify that the installed package bytes still match the hash recorded at import or update.
7. Require approval for that installed hash and reviewed access.
8. Apply runner policy, platform support, required-secret, and listener checks.
9. Resolve secrets only for execution and invoke the native implementation.

A failure at any stage blocks the related run. Approval cannot override malformed data, a package-hash mismatch, unsupported targets, missing secrets, or disabled dangerous-action policy.

Sensitive desktop operations also require a short-lived confirmation guard issued by the Rust backend. The guard is bound to the exact operation and reviewed content, expires after two minutes, and can be used only once. Changing a package or config after review invalidates the guard. The backend rejects missing, expired, reused, or mismatched guards even when the frontend sends a command directly.

## Package consistency and publisher identity

At import or update, the runner computes a SHA-256 hash of the complete `.bbs` file and stores it with the installed script record. Before status-sensitive operations and execution, it hashes the stored package again. A mismatch means the managed copy changed outside the runner and is no longer trusted.

A hash is a file fingerprint. BaudBound compares the current fingerprint with the one saved during import or update. If they differ, the runner blocks execution until the package is installed through the normal update and approval process.

This package hash detects local modification. It does **not** prove who authored or published the package. BaudBound does not currently provide package-author signatures. Obtain packages through a channel you trust, inspect their graph and access, and approve only the installed revision you intend to run.

Runner release signing is a separate system. Tauri updater signatures authenticate published runner update artifacts. They do not sign user-created `.bbs` packages.

## Approval and updates

Approval binds the installed script identity, current package hash, and reviewed access. Updating content changes the hash and makes the previous approval stale. Revocation removes the active approval without deleting the script.

Sub-scripts do not inherit their caller's trust. The called script must be installed, hash-valid, compatible, policy-allowed, and independently approved. This prevents an approved parent from smuggling an unreviewed package into execution.

Use [Approvals, Capabilities, and Risk](approvals-capabilities.md) for the complete review workflow and policy controls.

## Secrets

Packages contain declarations such as a secret name, type, description, and whether it is required. Production values are configured on the runner after import. Stored values are encrypted, omitted from normal inspection, and redacted from runtime logs and stored snapshots where they are exposed to execution.

The encryption key is part of the backup boundary. Database records without the matching key cannot be decrypted. See [Secrets](../runner/secrets.md) and [Storage, Backups, and Recovery](../runner/storage-backups.md).

## Network and native platform boundaries

Webhook and WebSocket listeners default to loopback. Each approved network trigger uses runner owned token protection by default. Approval displays newly created plaintext tokens once, then only their hashes remain in storage. Replacement tokens generated under Security are also displayed once. Browser requests require an exact configured origin. A non loopback bind is refused when any registered trigger has authentication disabled unless the operator enables the explicit unsafe override.

Token authentication does not provide TLS, rate limiting, or firewall policy. Put exposed listeners behind an appropriate reverse proxy and network controls.

Native actions use supported Rust libraries or operating-system APIs. The editor marks platform restrictions, package verification rejects incompatible targets, and runtime adapters reject unavailable behavior. BaudBound does not silently replace an unsupported native action with PowerShell, Bash, or desktop command-line tools.

## Operator checklist

- Download the runner only from the [official BaudBound GitHub Releases page](https://github.com/NATroutter/BaudBound/releases) and allow signature verification to complete.
- Validate and inspect a package before import when its source is unfamiliar.
- Review capabilities, permissions, target, risk, secrets, listeners, and variable-controlled paths.
- Re-review every updated revision instead of approving by filename.
- Keep listeners on loopback unless network exposure is intentionally designed.
- Back up the runner database and matching secret key securely.
- Run one service owner per runner home to avoid duplicate trigger delivery.
- Treat unexpected secret text in logs as a security defect.

Continue with [BBS Package Format](../package-format/index.md) for the archive contract or [Approvals, Capabilities, and Risk](approvals-capabilities.md) for operator review.

---
title: Security Model
description: BaudBound package integrity, capabilities, approvals, secrets, policy, and native execution boundaries.
tags: [security]
---
# Security Model

BaudBound treats imported automation as untrusted executable intent. The runner validates package structure and hashes, recalculates permissions and capabilities from the graph, enforces target-runtime compatibility, applies runner policy, and requires approval for the exact installed revision.

The editor's declarations are review material, not an authority the runner blindly trusts. Unknown action types, undeclared executable behavior, mismatched capability files, duplicate declarations, unsupported targets, and falsified risk are rejected.

Native operations are implemented through Rust libraries and platform APIs. BaudBound does not emulate unsupported native actions by constructing PowerShell, shell, or desktop-tool scripts. The Shell node is an explicit high-risk automation feature and is governed as such; it is not an implementation shortcut for other nodes.

Secrets are supplied by the runner operator, encrypted at rest, redacted from logs, and never included in exported packages. Network listeners default to loopback and remain subject to host firewall and proxy policy.

Read [Capabilities and approvals](permissions-capabilities.md), [Package integrity](package-integrity.md), and [Secrets](../runner/secrets.md).

---
title: Security Model
description: Package integrity, capabilities, approvals, secrets, policy, and execution boundaries.
tags: [security, approvals, integrity]
---
# Security Model

BaudBound treats imported automation as untrusted executable intent. The runner validates package structure and integrity, recalculates requested access from the graph, enforces target compatibility and policy, and requires approval for the installed revision.

Editor declarations are review material rather than authority. Unknown node types, mismatched permissions or capabilities, duplicate declarations, unsupported targets, and false risk classifications are rejected.

## Permissions, capabilities, and risk

Permissions are broad access categories. Capabilities describe more specific behavior requested by nodes. Package risk is the highest applicable classification across the graph.

During import and update, the runner recalculates these values and requires package declarations to match. A truthful high-risk package can be inspected, but execution remains blocked until approval and runner policy allow it.

Review shell commands, process control, filesystem mutation, network listeners, native input, desktop access, and serial I/O carefully. Unsupported native behavior is rejected for incompatible target runtimes. The Shell node is an explicit high-risk feature.

## Approval

Approval binds package identity, content hash, and reviewed access. It is not permanent trust in a filename. Package content or capability changes require another review.

Authorization is checked again before execution. A sub-script must independently satisfy validation and approval requirements.

## Package integrity

The runner verifies integrity metadata during import and checks the installed package against its recorded hash. Content changed outside the runner is not silently trusted. Make workflow changes in the editor and install a newly exported package.

## Secrets and network boundaries

Packages declare secret names but never contain production values. The runner encrypts stored values and redacts them from logs. See [Secrets](../runner/secrets.md) for desktop and headless setup.

Network listeners default to loopback. Exposing them on another interface also requires host firewall, proxy, and authentication decisions appropriate to the deployment.

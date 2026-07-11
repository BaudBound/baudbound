---
title: Permissions, Capabilities, and Approvals
description: How BaudBound derives access, classifies risk, applies policy, and records operator approval.
tags: [security, capabilities, approvals]
---
# Permissions, Capabilities, and Approvals

Permissions are coarse access categories. Capabilities are more specific descriptions of behavior requested by graph nodes. Risk is the highest applicable classification across the package.

During import and update, the runner recalculates these values from every known node type and requires package declarations to match exactly. This catches both hidden behavior and stale editor output. Validation may accept a truthful high-risk package into storage so the operator can inspect it; execution policy remains restrictive until approval and policy requirements are satisfied.

An approval binds package identity, cryptographic content hash, and reviewed access. It is invalidated by package changes, changed capabilities, or incompatible policy. Filename reuse does not preserve trust.

The runner checks authorization immediately before execution, not only at import time. Sub-scripts undergo their own validation and approval checks. Desktop actions also require a compatible interactive target and an available native adapter.

High-risk capabilities should be narrowly scoped. Shell commands, process control, filesystem mutation, network servers, native input injection, and sensitive desktop access deserve explicit review. The default policy can block dangerous actions, shell execution, or network server triggers even when a package is otherwise valid.

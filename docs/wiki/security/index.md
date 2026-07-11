---
title: Security Model
description: Package integrity, permissions, approvals, and secret handling in BaudBound.
tags:
  - security
---
# Security Model

BaudBound treats editor packages as untrusted input. The runner independently validates package structure, hashes, permissions, capabilities, target compatibility, and minimum runner versions before execution.

## Package approvals

Approvals are bound to the exact installed package hash and declared permissions. If a package changes, the previous approval no longer applies. Medium, high, and dangerous operations remain visible during review.

## Secrets

Packages declare secret names and types but never contain secret values. Values are configured on the runner, encrypted before durable storage, and exposed to scripts as read-only inputs. Stored values are not returned to the desktop React interface and are redacted from persisted reports.

## Updates

Desktop updates are downloaded through Tauri's updater and must match the public signing key embedded in the installed application. An unsigned or mismatched update is rejected before installation.

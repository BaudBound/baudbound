---
title: Scripts and Approvals
description: Manage installed packages, revisions, enablement, capabilities, and approvals.
tags: [runner, scripts, security]
---
# Scripts and Approvals

## Import and update

Import validates a package and installs it under its package identity and name.

Use `script update` to replace an existing package. The replacement must represent the same script identity. A content hash change invalidates the previous approval and requires review before execution.

## Package integrity status

**Verified** means every integrity entry matches package content. **Not verified** means the package contract lacks valid integrity proof or installed bytes no longer match. Unverified packages cannot be treated as trusted merely because they were previously approved.

## Approval state

The UI describes an approval as **Approved for this version** when the accepted hash and capability set match the installed revision. **Review required** means no approval exists or package content, capabilities, policy, or target compatibility changed.

Approval is per package revision, not a permanent trust grant to a filename. Review the author metadata, target runtime, requested capabilities, risk classification, secret declarations, and node inventory.

## Enablement

Enabled scripts are eligible for background trigger loading. Enablement does not bypass validation or approval. Disabled scripts remain installed and inspectable but are not loaded by `serve`.

Removing a script removes it from the installed script list. Review any run information you need before removal.

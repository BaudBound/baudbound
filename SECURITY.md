# Security Policy

## Reporting a vulnerability

Please report security problems privately through the
[BaudBound security advisory form](https://github.com/BaudBound/baudbound/security/advisories/new).

Include the affected version, operating system, reproduction steps, and the
impact you observed. Do not include real passwords, tokens, signing keys, or
private script data. Please do not open a public issue before the report has
been reviewed.

## Package trust and runner policy

BaudBound calculates a package's permissions from its program. Repository
metadata and package permission claims are never treated as authority.
Approval trusts one exact package revision. A changed package hash or
permission set requires another review.

The `[security.policy]` configuration is a separate operator controlled layer.
It can block shell commands, Dangerous permissions, and public network
listeners even when a package is approved. Packages and repositories cannot
change this policy.

## Runtime protection

BaudBound permits intentional endless workflows. It does not impose a maximum
run duration or loop count. It instead bounds pending work, retained logs,
retained values, one run record, and total run history. Exact arithmetic fails
before overflow. Diagnostic counters saturate when their precise value no
longer affects execution.

Limited relative file actions run inside a workspace at
`workspaces/<script-id>` under the runner home. Absolute paths, templated paths,
parent traversal, and paths outside that workspace require the corresponding
Dangerous permission. Symlink components that escape the workspace are
rejected.

HTTP diagnostics retain safe request metadata, response status, timing, sizes,
and SHA-256 body hashes. Sensitive headers, query values, structured secret
fields, and configured secret values are redacted. Previews and stored output
are bounded and visibly marked when truncated.

## Accepted residual risk

Limited file actions resolve and validate their path immediately before using
it. An attacker who can modify the runner's workspace at the same time may be
able to replace a validated path component with a link before the file
operation begins. Package permission calculation still requires Dangerous
approval for absolute, templated, and parent traversal paths. Workspace links
that already exist during validation are rejected.

This low severity race is accepted temporarily because Rust's portable
standard filesystem API does not provide directory handle relative operations
on both supported platforms. The BaudBound maintainer owns this exception. It
must be reviewed by 2026-10-25 or earlier if a suitable capability based
filesystem implementation becomes practical.

## Dependency policy

`deny.toml` is the source of truth for Rust advisories, licenses, sources, and
dependency bans. Both supported release targets must pass the complete policy.
Production UI dependencies must pass `pnpm audit --prod`.

An exception must name the dependency, reason, owner, and review date. A
temporary security advisory exception must also document whether the affected
code ships at runtime, the reachable attack surface, a removal condition, and
an expiry date. Build-only findings must not be described as runtime exposure.

## Release security

Every external GitHub Action is pinned to a complete commit SHA. Release
signing secrets belong in the protected `runner-release` GitHub environment,
not repository-level secrets. The signed package jobs and the final checksum
publication job reference that environment. Only the package jobs reference
the signing credentials. Read-only quality and artifact verification jobs do
not use the environment. Environment approval protects publication before a
write-capable job starts. SHA pinning protects approved jobs from mutable
third-party action tags.

Signed releases remain drafts until their metadata, native packages, updater
manifest, signatures, checksums, and installation tests pass. A maintainer
must inspect the draft before publication.

When updating an Action, review the upstream release and commit, replace the
SHA while keeping the readable version comment, and run the complete affected
workflow. Never replace a pinned SHA with a branch or mutable version tag.

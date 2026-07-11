---
title: Secrets
description: Declare, store, resolve, rotate, and safely simulate sensitive runner values.
tags: [runner, security, secrets]
---
# Secrets

Packages declare required secret names, types, and descriptions. They never contain production secret values. The runner resolves values at execution time and fails clearly when a required secret is unavailable.

## Desktop storage

Desktop builds store the encryption key through the operating system credential vault. Encrypted secret records are kept in runner durable storage. The UI and CLI never return secret plaintext in list or status responses.

## Headless storage

Generate a key once:

```text
baudbound secret generate-key
```

Provide the resulting key to the supervised runner through `BAUDBOUND_SECRET_KEY`, using the host's secret-management mechanism. Do not put it in `config.toml`, shell history, source control, service templates, or package assets.

Manage values with `baudbound secret set`, `secret list`, and `secret remove`. Values are encrypted with authenticated encryption before durable storage.

## Runtime handling

Resolved values are redacted from structured logs and error context. Secrets do not expose derived variable metadata. Updating or removing a value affects subsequent runs without modifying packages.

## Editor simulation

The editor may use placeholders or user-entered actual values for one simulation. Actual values remain ephemeral and are not saved to editor project state or exported packages.

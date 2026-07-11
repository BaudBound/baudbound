---
title: Secrets
description: Declare, store, resolve, rotate, and safely simulate sensitive runner values.
tags: [runner, security, secrets]
---
# Secrets

Packages declare required secret names, types, and descriptions. They never contain production secret values. The runner resolves values at execution time and fails clearly when a required secret is unavailable.

In the examples below, replace `SCRIPT` with the installed script name or ID and `SECRET_NAME` with a secret declared by that script. Find both values with:

```text
baudbound script list
baudbound secret list SCRIPT
```

## Desktop storage

Desktop builds store the encryption key through the operating-system credential vault. Encrypted secret records are kept in runner durable storage. The UI and CLI never return secret plaintext in list or status responses.

To configure a value in the desktop application, open **Security**, select the installed script, find the required secret, and choose its set or update action. Enter the value only into the secret prompt. The normal script details, runs, and logs do not display it.

## Headless storage

Generate a key once:

```text
baudbound secret generate-key
```

The command prints an assignment beginning with `BAUDBOUND_SECRET_KEY=`. Store that complete assignment in the protected environment file or secret manager used by the supervised service. The same value must be present whenever a CLI command reads or changes encrypted secrets for that runner home.

Do not put the key in `config.toml`, shell history, source control, public service templates, or package assets. Losing the key makes existing encrypted values unreadable; back it up through the same protected process used for other service credentials.

List declarations and whether each value is configured:

```text
baudbound secret list SCRIPT
```

Set a value. The command prompts without echoing the entered text:

```text
baudbound secret set SCRIPT SECRET_NAME
```

Remove a configured value:

```text
baudbound secret remove SCRIPT SECRET_NAME
```

Values are encrypted with authenticated encryption before durable storage. Removing a required value causes future runs to fail until another value is configured.

## Runtime handling

Resolved values are redacted from structured logs and error context. Secrets do not expose derived variable metadata. Updating or removing a value affects subsequent runs without modifying packages.

## Editor simulation

The editor may use placeholders or user-entered actual values for one simulation. Actual values remain ephemeral and are not saved to editor project state or exported packages.

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

## Lifecycle

1. Declare the secret's name, type, description, and required state in the editor.
2. Reference it as `{{SECRET_NAME}}` without entering the production value into project data.
3. Export and import the package.
4. Inspect the declaration and confirm its purpose.
5. Set the value on the intended runner.
6. Run the script and confirm no plaintext appears in output.
7. Rotate by setting a replacement value. Subsequent runs use it.
8. Remove the value when access is revoked or the script no longer needs it.

Values are scoped by stable script identity and secret name. Two scripts declaring `api_token` have separate encrypted records.

## Desktop storage

Windows desktop installations protect the encryption key through Windows Credential Manager.

Linux desktop installations use the standard Secret Service interface. GNOME Keyring, KWallet, and other compatible credential managers can provide this interface. Follow [Linux encrypted secret storage](installation.md#linux-encrypted-secret-storage) before configuring production secrets.

The credential service must be available in the same graphical user session as BaudBound. This matters when using VNC or another remote desktop session because it may not share the credential service started by a local login.

BaudBound connects to the credential vault in the background so a slow provider cannot delay the desktop window. The Security page displays the current connection state and provides a Retry button after a failure.

When the credential vault is unavailable, BaudBound does not replace it with an unencrypted file. The desktop application continues without secret access and scripts requiring secrets cannot resolve their values.

Encrypted secret records are kept in runner durable storage. The UI and CLI never return secret plaintext in list or status responses.

To configure a value in the desktop application, open **Security**, select the installed script, find the required secret, and choose its set or update action. Enter the value only into the secret prompt. The normal script details, runs, and logs do not display it.

## Headless storage

Generate a key once:

```text
baudbound secret generate-key
```

The command prints an assignment beginning with `BAUDBOUND_SECRET_KEY=`. Store that complete assignment in the protected environment file or secret manager used by the supervised service. The same value must be present whenever a CLI command reads or changes encrypted secrets for that runner home.

Do not put the key in `config.toml`, shell history, source control, public service templates, or package assets. Losing the key makes existing encrypted values unreadable. Back it up through the same protected process used for other service credentials.

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

When using a system service, restart it after changing the environment file so the process receives the key. Interactive CLI commands must receive the same environment value, or they report that the secret key is unavailable.

## Runtime handling

Resolved values are redacted from structured logs and error context. Secrets do not expose derived variable metadata. Updating or removing a value affects subsequent runs without modifying packages.

Redaction removes exact secret values from runtime variables and replaces matching text in logs and nested report values. It is defense in depth, not permission to send secrets to arbitrary actions. A secret can intentionally leave the runner through HTTP, typed text, clipboard, files, processes, shell, serial, or another configured side effect.

Never place plaintext in:

- project variables, node defaults, comments, or assets.
- `.bbs` package files.
- `config.toml`.
- command-line arguments or shell history.
- log messages or error descriptions.
- screenshots or support bundles.
- source control and CI output.

Sub-script execution does not transfer the parent's secret records. The child script resolves its own declarations under its own identity and must be independently configured and approved.

## Rotate a value

1. Create or obtain the replacement from its authoritative system.
2. Set it with the desktop protected prompt or `baudbound secret set SCRIPT SECRET_NAME`.
3. Run a controlled test and confirm success.
4. Revoke the old credential at its provider.
5. Confirm logs and reports contain no plaintext.

Setting uses an authenticated-encryption nonce and replaces the existing encrypted record atomically at the database level.

## Backup and key loss

Back up the headless environment key separately from the runner home. For desktop installations, use the operating system's supported credential-vault migration or backup process.

The database contains ciphertext and nonce, not a recoverable copy of the encryption key. If the key is lost, existing values cannot be decrypted. Configure each declaration again from its authoritative source. Do not attempt to bypass encryption by editing the database.

## Editor simulation

The editor may use placeholders or user-entered actual values for one simulation. Actual values remain ephemeral and are not saved to editor project state or exported packages.

Actual simulation values can still be transmitted by real browser HTTP Request simulation. Prefer test credentials and review the complete graph before entering production data. See [Verification and Simulation](../editor/simulation.md#secrets-in-simulation).

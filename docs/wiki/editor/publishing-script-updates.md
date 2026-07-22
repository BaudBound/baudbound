---
title: Publishing Script Updates
description: Publish versioned BaudBound packages and a matching update descriptor through GitHub or another public HTTPS host.
tags: [editor, scripts, updates, publishing]
---
# Publishing Script Updates

BaudBound can tell a runner that a newer script package exists. The runner discovers the package through a small file named `update.json`.

An update check never installs, approves, enables, or runs a package. The operator must review the downloaded package and approve its exact new hash before it can run.

## What you publish

Publish these two files:

1. The exported `.bbs` package contains the script itself.
2. `update.json` tells the runner which version is current, where to download it, and which exact bytes to expect.

Both files must be available through public HTTPS addresses. BaudBound does not currently support private feeds, authentication, local network hosts, or plain HTTP.

## Prepare the project

Open **Settings** in the editor and complete these fields:

| Field | What to enter |
| --- | --- |
| **Script version** | A semantic version such as `1.0.0` or `1.2.0` |
| **Update URL** | The final public address where `update.json` will be available |
| **Project ID** | Read only. Keep this value unchanged for every release of the same script |

The update URL must use HTTPS and its filename must be `update.json`. It is allowed to point to a file that does not exist yet. This makes the first release possible without creating a temporary descriptor.

Increase the script version whenever the package changes. Use `1.0.1` for a compatible fix, `1.1.0` for a compatible feature, and `2.0.0` for an incompatible change.

BaudBound refuses a published package that uses an older version. It also refuses the same version when the package hash has changed. This prevents a publisher from silently replacing already reviewed release bytes.

## Export the package and descriptor

1. Choose **Export**.
2. Review the project information and calculated access.
3. Complete verification.
4. Choose **Prepare export** when the final step requests it.
5. Choose **Download package** to save the generated `.bbs` file.
6. Enter the final public package URL.
7. Add release notes when useful.
8. Choose **Create update.json**.

The editor calculates the descriptor size and SHA256 from the same generated package bytes offered by **Download package**. Keep the export window open until both files are created. Closing it discards the generated temporary state.

## Descriptor fields

```json
{
  "format": "baudbound.script-update",
  "format_version": 1,
  "script_id": "6db0f09c-2d76-4ea3-bb6b-9a093a04d8f7",
  "latest": {
    "version": "1.2.0",
    "package_url": "https://github.com/USERNAME/REPOSITORY/releases/download/v1.2.0/example-1.2.0.bbs",
    "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    "size": 123456,
    "published_at": "2026-07-22T12:00:00Z",
    "release_notes": "Adds the new inventory workflow."
  }
}
```

| Field | Meaning |
| --- | --- |
| `format` | Identifies this file as a BaudBound script update descriptor |
| `format_version` | Identifies the descriptor contract understood by the runner |
| `script_id` | Must exactly match the stable project ID inside the package |
| `latest.version` | Must exactly match the script version inside the package |
| `latest.package_url` | Public HTTPS address of the actual `.bbs` file |
| `latest.sha256` | Lowercase SHA256 of the exact package bytes |
| `latest.size` | Exact package size in bytes |
| `latest.published_at` | Publication time in UTC |
| `latest.release_notes` | Markdown text shown by the runner |

The package URL must download the `.bbs` file. Do not use a GitHub page that displays information about the release.

## Publish with GitHub

One simple arrangement uses a GitHub Release for the package and GitHub Pages for the stable descriptor.

1. Create a GitHub Release whose tag matches the script version, such as `v1.2.0`.
2. Upload the `.bbs` package as a release asset.
3. Copy the direct asset URL into the editor export window.
4. Create `update.json` from that exact generated package.
5. Put `update.json` in the branch and directory published by GitHub Pages.
6. Confirm that opening the configured update URL returns JSON.
7. Confirm that opening the package URL downloads the `.bbs` file.

The public addresses can look like these:

```text
https://USERNAME.github.io/REPOSITORY/update.json
```

```text
https://github.com/USERNAME/REPOSITORY/releases/download/v1.2.0/example-1.2.0.bbs
```

For the next release, upload the new package first and then replace `update.json`. Publishing the descriptor last prevents runners from discovering a package that is not available yet.

## Verify the package hash yourself

On Windows PowerShell, run:

```powershell
Get-FileHash .\example-1.2.0.bbs -Algorithm SHA256
```

On Linux, run:

```text
sha256sum ./example-1.2.0.bbs
```

Compare the lowercase hash with `latest.sha256`. Also confirm that the descriptor size matches the file size.

## What the runner checks

The runner performs these checks before presenting an update for review:

1. The descriptor and package URLs use public HTTPS destinations.
2. Redirects remain on allowed public HTTPS destinations.
3. Responses stay within configured byte limits and timeouts.
4. The descriptor follows the supported schema.
5. The script ID and version match the package.
6. The downloaded package size and SHA256 match the descriptor.
7. The package passes the normal schema, compatibility, permission, capability, and integrity checks.
8. The version is newer than the installed version.

The runner does not send script secrets, browser credentials, cookies, or authentication headers to the publisher. The publisher can still observe the runner's public IP address and the time of each request.

> Install scripts only from publishers you trust. BaudBound can validate format, integrity, compatibility, declared access, and exact package bytes. It cannot determine whether a publisher or a script has good intentions.
{.is-warning}

## Common failures

| Message | What to check |
| --- | --- |
| Update URL is not configured | Add the final `update.json` HTTPS address in Project Settings and export again |
| Descriptor belongs to another script | Export from the original saved project so its stable ID is retained |
| Descriptor version does not match | Create `update.json` from the same export operation as the uploaded package |
| Package hash or size does not match | Replace the package or recreate the descriptor from the exact uploaded bytes |
| Version is not newer | Increase the script version before exporting the changed package |
| Same version has different bytes | Publish the changed package with a new semantic version |
| Destination is restricted | Use a public host instead of localhost, a private address, or a local network server |
| Remote request failed | Confirm that both public URLs work and try the check again |

See [Script Management](../runner/script-management.md#remote-import-and-update-checks) for the runner controls used to discover and review published updates.

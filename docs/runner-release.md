# Runner Release Process

BaudBound desktop releases are built only by the tag-gated GitHub Actions workflow. The workflow creates a draft GitHub Release containing:

- a Windows NSIS installer;
- a Linux AppImage built on Ubuntu 22.04;
- a Debian package for 64-bit Debian and Ubuntu;
- an RPM package for 64-bit Fedora;
- Tauri updater signatures; and
- `latest.json`, which points self-updating desktop installations to the signed platform artifact; and
- `SHA256SUMS`, which records the installer and package checksums.

## Signing Identity

The updater signing private key is release-critical. Losing the key or its password prevents existing installations from accepting future updates. Keep an offline backup of both values before publishing the first release.

Configure these GitHub Actions repository secrets:

- `TAURI_SIGNING_PRIVATE_KEY`: the complete contents of the Tauri updater private key;
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: the key password.

The matching public key is intentionally committed in `apps/baudbound/tauri.conf.json`. It is not secret and is used only to verify update signatures.

Updater signing and Windows publisher code signing are separate. The first release uses cryptographically signed Tauri updates but does not claim a Windows publisher certificate. The NSIS installer may therefore show the normal Windows warning for an unsigned publisher. Add Authenticode signing only after obtaining and securely provisioning a suitable certificate.

## Release Versions

BaudBound uses semantic versions:

- Patch: bug fixes and compatible internal changes, for example `2.0.0` to `2.0.1`.
- Minor: compatible user-facing features, for example `2.0.1` to `2.1.0`.
- Major: package, configuration, security, or behavior changes that require migration, for example `2.1.0` to `3.0.0`.

The release version must match in all three locations:

- workspace `Cargo.toml` under `[workspace.package]`;
- `apps/baudbound/tauri.conf.json`;
- `apps/baudbound/ui/package.json`.

The version gate rejects a release tag if any value differs.

## Prepare A Release

The examples below use PowerShell and version `2.0.1` to demonstrate an update. Use `2.0.0` when preparing the first public release, or replace the example with the intended later version.

### Release helper

The repository includes a guarded PowerShell helper for the repeatable release operations. Run it without arguments to open an interactive arrow-key menu with a description of each operation:

```powershell
./tools/runner-release.ps1
```

Use the Up and Down arrow keys to select an operation, press Enter to continue, or press Escape to exit. The helper asks for the release version and defaults to the current version in `tauri.conf.json`.

For automation or direct invocation, define:

```powershell
$releaseTool = "./tools/runner-release.ps1"
$version = "2.0.1"
```

The normal assisted release flow is:

```powershell
# After manually updating the three version files:
& $releaseTool -Action Verify -Version $version

# After reviewing, committing, and pushing the release commit:
& $releaseTool -Action Tag -Version $version -ConfirmTag
& $releaseTool -Action Watch -Version $version

# Retry failed Runner CI or Runner Release workflow jobs:
& $releaseTool -Action Retry -Version $version

# After CI creates the draft release:
& $releaseTool -Action Inspect -Version $version

# Only after manually testing every downloaded platform package:
& $releaseTool -Action Publish -Version $version -ConfirmPublish
```

`Tag` refuses to proceed unless the worktree is clean, the current branch is `master`, all versions agree, and any existing remote tag points to the exact release commit. It also requires Runner CI to pass for that commit. When GitHub path filtering skipped Runner CI, the helper starts it with `workflow_dispatch` and waits for it. A failed tag push can be retried because the helper safely reuses a matching local or remote tag. `Retry` handles Runner CI until the tag reaches GitHub. After the remote tag exists, it handles Runner Release instead. `Publish` requires the release workflow to have passed, downloads the draft, and validates its structure again. Use `-Confirm:$false` only in a controlled non-interactive environment where the explicit confirmation switch has already been reviewed.

When an interactive operation fails, the helper always lets you retry that operation or exit without undoing completed steps. Operations associated with GitHub Actions also offer a workflow retry. The helper waits for newly dispatched workflows to appear instead of failing during GitHub's short registration delay.

The remaining sections describe every operation in detail and provide the manual commands for recovery or auditing.

### 1. Update the release branch

Start from a clean checkout of the release branch:

```powershell
git switch master
git pull --ff-only origin master

if (git status --porcelain) {
    throw "The worktree is not clean. Commit or remove unrelated changes before releasing."
}
```

Do not prepare a release from an unreviewed feature branch or a dirty worktree.

### 2. Set the version

Set the intended version in the three files listed above, then verify the complete version contract:

```powershell
$version = "2.0.1"
$tag = "v$version"

node apps/baudbound/scripts/verify-release-version.mjs $tag
```

The command must print:

```text
Release versions agree on 2.0.1.
```

Refresh lock metadata after changing the workspace version:

```powershell
cargo check -p baudbound
pnpm --dir apps/baudbound/ui install --lockfile-only
```

### 3. Run the local release gate

Install exact locked dependencies and run every automated contract used by the release workflow:

```powershell
pnpm --dir apps/baudbound/ui install --frozen-lockfile
pnpm --dir apps/editor install --frozen-lockfile

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked

pnpm --dir apps/baudbound/ui test
pnpm --dir apps/baudbound/ui build

pnpm --dir apps/editor schemas:check
pnpm --dir apps/editor test

git diff --check
```

Any failure blocks the release. Fix the underlying problem and rerun the complete gate.

### 4. Optionally build the signed Windows installer locally

The local updater password is stored with Windows DPAPI. This command decrypts it only in memory, builds the NSIS installer, and clears the signing environment afterward:

```powershell
$keyPath = Join-Path $HOME ".tauri/baudbound-updater.key"
$passwordPath = Join-Path $HOME ".tauri/baudbound-updater.password.dpapi"
$securePassword = Get-Content -LiteralPath $passwordPath | ConvertTo-SecureString
$credential = [PSCredential]::new("unused", $securePassword)
$password = $credential.GetNetworkCredential().Password

try {
    $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content -Raw -LiteralPath $keyPath
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $password

    Push-Location apps/baudbound
    try {
        pnpm dlx @tauri-apps/cli@2 build --bundles nsis --ci
        if ($LASTEXITCODE -ne 0) {
            throw "The local Tauri release build failed with exit code $LASTEXITCODE."
        }
    } finally {
        Pop-Location
    }
} finally {
    Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY -ErrorAction SilentlyContinue
    Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
    $password = $null
    $credential = $null
    $securePassword = $null
}
```

Confirm both files exist:

```powershell
Get-Item `
  "target/release/bundle/nsis/BaudBound_${version}_x64-setup.exe", `
  "target/release/bundle/nsis/BaudBound_${version}_x64-setup.exe.sig"
```

Never put the private key, password, decrypted password, or a plaintext `.env` containing either value in the repository.

### 5. Commit and run normal CI

Review the release changes before committing:

```powershell
git status --short
git diff
git diff --check
```

Commit and push the version update:

```powershell
git add Cargo.toml Cargo.lock apps/baudbound/tauri.conf.json apps/baudbound/ui/package.json
git commit -m "release: prepare BaudBound $tag"
git push origin master
```

Wait for the normal `Runner CI` workflow to pass before creating the tag. With GitHub CLI installed:

```powershell
gh auth status
gh run list --workflow runner-ci.yml --commit (git rev-parse HEAD)
```

GitHub can skip Runner CI when the release commit only changes files outside the workflow path filters. In that case, start Runner CI manually for `master`:

```powershell
gh workflow run runner-ci.yml --ref master
```

The release helper performs this check and dispatch automatically.

Open the matching run or watch it by ID:

```powershell
$ciRun = gh run list `
  --workflow runner-ci.yml `
  --commit (git rev-parse HEAD) `
  --limit 1 `
  --json databaseId `
  --jq '.[0].databaseId'

gh run watch $ciRun --exit-status
```

### 6. Create and push the release tag

Confirm `HEAD` is the tested release commit, then create an annotated tag:

```powershell
git status --short
git log -1 --oneline
node apps/baudbound/scripts/verify-release-version.mjs $tag

git tag -a $tag -m "BaudBound $tag"
git push origin $tag
```

Pushing the tag starts `.github/workflows/runner-release.yml`. The workflow reruns the full quality gate, builds the Windows NSIS installer and the Linux AppImage, Debian, and RPM packages, signs updater artifacts, generates `latest.json`, and creates a draft GitHub Release.

## Monitor The Release Build

Find and watch the workflow associated with the tagged commit:

```powershell
$releaseRun = gh run list `
  --workflow runner-release.yml `
  --commit (git rev-list -n 1 $tag) `
  --limit 1 `
  --json databaseId `
  --jq '.[0].databaseId'

gh run watch $releaseRun --exit-status
```

Watch returns a nonzero exit code when the workflow fails. This means the workflow reported a failure, not that watching stopped working. The release helper reads the completed run and displays the failed job and step in its recovery screen.

If it fails, inspect only the failed logs:

```powershell
gh run view $releaseRun --log-failed
```

For a transient runner or network failure, rerun failed jobs without changing the tag:

```powershell
gh run rerun $releaseRun --failed
gh run watch $releaseRun --exit-status
```

Do not rerun a deterministic compilation, test, version, or signing failure without fixing its cause.

The helper detects whether the failure belongs to Runner CI or Runner Release, performs the recovery, and then watches the new attempt:

```powershell
& $releaseTool -Action Retry -Version $version
```

## Inspect The Draft Release

Confirm the release exists and remains a draft:

```powershell
gh release view $tag --json tagName,name,isDraft,isPrerelease,assets
```

Download every release asset into a clean temporary directory:

```powershell
$releaseDirectory = Join-Path $env:TEMP "baudbound-$version-release"
Remove-Item -LiteralPath $releaseDirectory -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $releaseDirectory | Out-Null

gh release download $tag --dir $releaseDirectory
Get-ChildItem $releaseDirectory | Select-Object Name,Length
```

The draft must contain at least:

- a Windows NSIS setup executable;
- its `.sig` updater signature;
- a Linux AppImage;
- its `.sig` updater signature; and
- a Debian `.deb` package;
- an `.rpm` package;
- `latest.json`; and
- `SHA256SUMS` containing all four installable artifacts.

Inspect the updater manifest:

```powershell
$manifest = Get-Content -Raw (Join-Path $releaseDirectory "latest.json") | ConvertFrom-Json

if ($manifest.version -ne $version) {
    throw "latest.json version does not match $version"
}

$manifest.platforms.PSObject.Properties | ForEach-Object {
    if (-not $_.Value.url -or -not $_.Value.signature) {
        throw "Updater platform $($_.Name) is missing its URL or signature"
    }
    [PSCustomObject]@{
        Platform = $_.Name
        URL = $_.Value.url
        HasSignature = [bool]$_.Value.signature
    }
}
```

Do not manually edit `latest.json` or any `.sig` file. They are generated from the signed CI artifacts.

## Validate Release Artifacts

Before publishing the draft:

1. Install the NSIS package on a clean Windows test environment.
2. Confirm shortcuts, first launch, config initialization, CLI commands, and uninstall registration.
3. Install the Debian package with APT on a clean supported Debian or Ubuntu test environment.
4. Install the RPM package with DNF on a clean supported Fedora test environment.
5. Confirm both native packages add the application-menu launcher and `baudbound` command.
6. Run the AppImage on the supported Linux test environments.
7. Confirm every installation reports the expected version:

```powershell
baudbound --version
```

8. Import, approve, and run a small known package.
9. Start and stop the desktop background runner.
10. Import a package with Webhook and WebSocket triggers, approve it, and confirm the one time token dialog shows each new token before the values become unrecoverable.
11. Confirm a Webhook request without `X-BaudBound-Token` receives `401` and the current token succeeds.
12. Confirm a WebSocket handshake without a token is rejected and the current token succeeds.
13. Confirm a public bind refuses to start when one matching trigger has authentication disabled and the unsafe override is off.
14. Confirm oversized HTTP responses, downloads, and file reads fail without replacing an existing download destination.
15. Confirm Run Process is shown as Dangerous and requires approval for the current revision.
16. Upgrade each native package by installing the newer file over the old version, then remove and reinstall it.
17. Confirm package upgrades and removal preserve the runner data in the user profile.
18. Confirm Debian and RPM installations show package-manager update instructions instead of in-app installation buttons.
19. Review generated release notes and remove unrelated or misleading text.

The release notes must explain that network trigger tokens are runner owned, approval shows new values once, and generating a replacement shows that value only once. They must also call out the exact browser Origin allowlist, public bind refusal, Run Process risk change, and external data limits. Existing pre-release integrations must be updated with a current token before they can call a protected trigger.

The release remains invisible to the production updater while it is a draft.

## Publish The Release

Publish and explicitly mark the release as latest only after all platform artifacts pass validation:

```powershell
gh release edit $tag --draft=false --latest
gh release view $tag --json tagName,isDraft,isPrerelease,publishedAt,url
```

Publishing activates this updater endpoint:

```text
https://github.com/NATroutter/BaudBound/releases/latest/download/latest.json
```

Verify the public endpoint returns the expected version:

```powershell
$publicManifest = Invoke-RestMethod `
  "https://github.com/NATroutter/BaudBound/releases/latest/download/latest.json"

if ($publicManifest.version -ne $version) {
    throw "The public updater manifest does not point to $version"
}
```

For releases after `2.0.0`, launch the previous installed desktop version and confirm:

1. the update modal appears;
2. release notes are correct;
3. progress reaches completion;
4. **Restart and install** stops the background runner cleanly; and
5. the restarted application reports the new version.

## Failed Release And Rollback Rules

An unpublished draft and its tag can be removed from the interactive menu. For direct invocation, run:

```powershell
& $releaseTool `
  -Action Remove `
  -Version $version `
  -ConfirmRemoveDraft
```

The helper refuses to remove a published release. PowerShell shows a confirmation before removal. The helper cancels active release workflow runs first, waits for them to stop, removes the draft and its assets, removes the remote tag, and then removes the local tag. The source commit remains on `master`.

- Never move, recreate, or overwrite a tag after its release has been published.
- Never replace an asset or signature in a published release.
- If a published release is broken, fix the problem and publish a newer patch version.
- Tauri rejects downgrades by default, so rollback is performed by publishing a newer version containing the previous working behavior.
- If a draft fails before publication, prefer fixing the issue and using a new patch version. Only delete and recreate an unpublished tag when you are certain nobody has consumed it.
- Never rotate the updater signing key unless a planned migration strategy exists. Existing installations trust the currently embedded public key.

After publication, keep the source commit, tag, release assets, `latest.json`, signing key backup, and release notes as permanent release records.

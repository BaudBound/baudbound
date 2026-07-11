function Get-ReleaseDownloadDirectory {
    if ($DownloadDirectory) {
        if ([IO.Path]::IsPathRooted($DownloadDirectory)) {
            return [IO.Path]::GetFullPath($DownloadDirectory)
        }
        return [IO.Path]::GetFullPath((Join-Path $script:RepositoryRoot $DownloadDirectory))
    }
    $timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
    return Join-Path ([IO.Path]::GetTempPath()) "baudbound-$script:Tag-$timestamp"
}

function Assert-ReleaseManifest {
    param([Parameter(Mandatory)][string]$Directory)
    $manifestPath = Join-Path $Directory "latest.json"
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
        throw "The draft release does not contain latest.json."
    }
    $manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
    if ($manifest.version -ne $Version) {
        throw "latest.json version '$($manifest.version)' does not match '$Version'."
    }
    $platforms = @($manifest.platforms.PSObject.Properties)
    if ($platforms.Count -lt 2) {
        throw "latest.json must contain Windows and Linux updater platforms."
    }
    foreach ($platform in $platforms) {
        if (-not $platform.Value.url -or -not $platform.Value.signature) {
            throw "Updater platform '$($platform.Name)' is missing its URL or signature."
        }
        $uri = $null
        $validUri = [Uri]::TryCreate([string]$platform.Value.url, [UriKind]::Absolute, [ref]$uri)
        if (-not $validUri -or $uri.Scheme -ne "https") {
            throw "Updater platform '$($platform.Name)' does not use an absolute HTTPS URL."
        }
    }
    $platformNames = $platforms.Name
    if (-not ($platformNames -match "windows") -or -not ($platformNames -match "linux")) {
        throw "latest.json requires Windows and Linux entries. Found: $($platformNames -join ', ')."
    }
}

function Inspect-DraftRelease {
    Require-Command "gh"
    Write-Step "Checking draft release metadata"
    $releaseJson = Invoke-ExternalCapture "gh" @(
        "release", "view", $script:Tag,
        "--json", "tagName,name,isDraft,isPrerelease,assets,url"
    )
    $release = $releaseJson | ConvertFrom-Json
    if (-not $release.isDraft) {
        throw "Release '$script:Tag' is already published and is not an inspectable draft."
    }
    if ($release.isPrerelease) {
        throw "Release '$script:Tag' is unexpectedly marked as a prerelease."
    }

    $directory = Get-ReleaseDownloadDirectory
    if (Test-Path -LiteralPath $directory) {
        if (@(Get-ChildItem -LiteralPath $directory -Force).Count -gt 0) {
            throw "Download directory '$directory' is not empty. Choose another directory."
        }
    } else {
        New-Item -ItemType Directory -Path $directory -Force | Out-Null
    }

    Write-Step "Downloading draft artifacts to $directory"
    Invoke-External "gh" @("release", "download", $script:Tag, "--dir", $directory)
    foreach ($pattern in @("*.exe", "*.exe.sig", "*.AppImage", "*.AppImage.sig", "latest.json")) {
        if (-not (Get-ChildItem -LiteralPath $directory -Filter $pattern -File)) {
            throw "The draft release is missing an artifact matching '$pattern'."
        }
    }
    Assert-ReleaseManifest -Directory $directory

    Write-Host "`nDraft metadata and updater artifacts passed structural validation." -ForegroundColor Green
    Get-ChildItem -LiteralPath $directory -File | Sort-Object Name | Format-Table Name, Length
    Write-Host "Artifacts: $directory"
    Write-Host "Manual platform installation and update testing are still required."
}

function Publish-Release {
    if (-not $ConfirmPublish) {
        throw "Publication requires -ConfirmPublish after Windows and Linux testing."
    }
    Inspect-DraftRelease
    if ($script:ReleaseCmdlet.ShouldProcess("GitHub release $script:Tag", "Publish and mark as latest")) {
        Write-Step "Publishing $script:Tag"
        Invoke-External "gh" @("release", "edit", $script:Tag, "--draft=false", "--latest")
        $endpoint = "https://github.com/NATroutter/BaudBound/releases/latest/download/latest.json"
        Write-Step "Verifying the public updater manifest"
        $publicVersion = $null
        for ($attempt = 1; $attempt -le 6; $attempt++) {
            $publicVersion = (Invoke-RestMethod -Uri $endpoint).version
            if ($publicVersion -eq $Version) {
                break
            }
            if ($attempt -lt 6) {
                Write-Host "  Attempt $attempt returned '$publicVersion'; retrying in 10 seconds."
                Start-Sleep -Seconds 10
            }
        }
        if ($publicVersion -ne $Version) {
            throw "The public updater manifest reports '$publicVersion', expected '$Version'."
        }
        Write-Host "`nPublished $script:Tag and verified its updater manifest." -ForegroundColor Green
    }
}

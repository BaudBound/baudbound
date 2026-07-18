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

function Assert-ReleaseArtifacts {
    param(
        [Parameter(Mandatory)][string]$Directory,
        [Parameter(Mandatory)][string]$ReleaseAssetsPath
    )
    $validator = Join-Path $script:RepositoryRoot "apps/baudbound/scripts/verify-release-assets.mjs"
    Invoke-External "node" @(
        $validator,
        $Directory,
        $script:Tag,
        "NATroutter/BaudBound",
        $ReleaseAssetsPath
    )
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
    $releaseAssetsPath = "$directory-release-assets.json"
    try {
        [IO.File]::WriteAllText(
            $releaseAssetsPath,
            $releaseJson,
            [Text.UTF8Encoding]::new($false)
        )
        Assert-ReleaseArtifacts `
            -Directory $directory `
            -ReleaseAssetsPath $releaseAssetsPath
    } finally {
        Remove-Item -LiteralPath $releaseAssetsPath -Force -ErrorAction SilentlyContinue
    }

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

function Select-ReleaseAction {
    $options = @(
        [PSCustomObject]@{ Value = "Verify"; Label = "Verify release"; Description = "Run all local release quality gates." },
        [PSCustomObject]@{ Value = "Tag"; Label = "Create release tag"; Description = "Validate CI, then create and push the version tag." },
        [PSCustomObject]@{ Value = "Watch"; Label = "Watch release build"; Description = "Follow the tag-triggered GitHub release workflow." },
        [PSCustomObject]@{ Value = "Inspect"; Label = "Inspect draft"; Description = "Download and validate draft release artifacts." },
        [PSCustomObject]@{ Value = "Publish"; Label = "Publish release"; Description = "Validate and publish the tested draft as latest." },
        [PSCustomObject]@{ Value = $null; Label = "Exit"; Description = "Close the release helper without making changes." }
    )
    return Select-TerminalMenu -Title "BaudBound runner release" -Options $options
}

function Read-ReleaseVersion {
    param([Parameter(Mandatory)][string]$RepositoryRoot)

    $configPath = Join-Path $RepositoryRoot "apps/baudbound/tauri.conf.json"
    $currentVersion = (Get-Content -Raw -LiteralPath $configPath | ConvertFrom-Json).version
    $versionPattern = '^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$'

    while ($true) {
        $value = Read-Host "Release version [$currentVersion]"
        if (-not $value) {
            return $currentVersion
        }
        if ($value -match $versionPattern) {
            return $value
        }
        Write-Host "Enter a semantic version without the v prefix, for example 2.0.1." -ForegroundColor Red
    }
}

function Select-ReleaseAction {
    $options = @(
        [PSCustomObject]@{ Value = "Verify"; Label = "Verify release"; Description = "Run all local release quality gates." },
        [PSCustomObject]@{ Value = "Tag"; Label = "Create release tag"; Description = "Validate CI, then create and push the version tag." },
        [PSCustomObject]@{ Value = "Watch"; Label = "Watch release build"; Description = "Follow the tag-triggered GitHub release workflow." },
        [PSCustomObject]@{ Value = "Retry"; Label = "Retry failed workflow"; Description = "Rerun failed Runner CI or release workflow jobs." },
        [PSCustomObject]@{ Value = "Inspect"; Label = "Inspect draft"; Description = "Download and validate draft release artifacts." },
        [PSCustomObject]@{ Value = "Publish"; Label = "Publish release"; Description = "Validate and publish the tested draft as latest." },
        [PSCustomObject]@{ Value = "Remove"; Label = "Remove draft and tag"; Description = "Cancel active builds and remove an unpublished release tag." },
        [PSCustomObject]@{ Value = $null; Label = "Exit"; Description = "Close the release helper without making changes." }
    )
    return Select-TerminalMenu -Title "BaudBound runner release" -Options $options
}

function Select-ReleaseFailureAction {
    param(
        [Parameter(Mandatory)][string]$Action,
        [Parameter(Mandatory)][string]$Message
    )
    $options = @(
        [PSCustomObject]@{ Value = "RetryOperation"; Label = "Retry operation"; Description = "Run the failed $Action operation again." },
        [PSCustomObject]@{ Value = $null; Label = "Exit"; Description = "Leave the helper without changing completed work." }
    )
    if (Test-ReleaseActionSupportsWorkflowRetry -Action $Action) {
        $workflowRetry = [PSCustomObject]@{
            Value = "RetryWorkflow"
            Label = "Retry failed workflow"
            Description = "Rerun the related GitHub workflow and resume this operation."
        }
        $options = @($options[0], $workflowRetry, $options[1])
    }
    return Select-TerminalMenu `
        -Title "BaudBound release operation failed" `
        -Details @("$Action failed", $Message) `
        -Options $options
}

function Test-ReleaseActionSupportsWorkflowRetry {
    param([Parameter(Mandatory)][string]$Action)

    return $Action -in @("Tag", "Watch", "Inspect", "Publish")
}

function Read-ReleaseVersion {
    param([Parameter(Mandatory)][string]$RepositoryRoot)

    $configPath = Join-Path $RepositoryRoot "apps/baudbound/tauri.conf.json"
    $currentVersion = (Get-Content -Raw -LiteralPath $configPath | ConvertFrom-Json).version
    $versionPattern = '^\d+\.\d+\.\d+$'

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

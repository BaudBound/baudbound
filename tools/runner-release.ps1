<#
.SYNOPSIS
Runs guarded BaudBound runner release operations.

.DESCRIPTION
Automates repeatable runner release work while keeping version edits, commits,
platform acceptance testing, and publication as explicit operator decisions.

.EXAMPLE
./tools/runner-release.ps1

.EXAMPLE
./tools/runner-release.ps1 -Action Verify -Version 2.0.0

.EXAMPLE
./tools/runner-release.ps1 -Action Publish -Version 2.0.0 -ConfirmPublish
#>
[CmdletBinding(SupportsShouldProcess, ConfirmImpact = "High")]
param(
    [ValidateSet("Verify", "Tag", "Watch", "Retry", "Inspect", "Publish", "Remove")]
    [string]$Action,

    [ValidatePattern('^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$')]
    [string]$Version,

    [ValidateNotNullOrEmpty()]
    [string]$ReleaseBranch = "master",

    [string]$DownloadDirectory,
    [switch]$ConfirmTag,
    [switch]$ConfirmPublish,
    [switch]$ConfirmRemoveDraft
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# Windows PowerShell otherwise decodes piped native output with the active OEM code page.
$previousConsoleInputEncoding = [Console]::InputEncoding
$previousConsoleOutputEncoding = [Console]::OutputEncoding
$previousOutputEncoding = $OutputEncoding
$utf8Encoding = [Text.UTF8Encoding]::new($false)
[Console]::InputEncoding = $utf8Encoding
[Console]::OutputEncoding = $utf8Encoding
$OutputEncoding = $utf8Encoding

$script:RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$script:ReleaseWorkflow = "runner-release.yml"
$script:CiWorkflow = "runner-ci.yml"
$script:ReleaseCmdlet = $PSCmdlet

$script:ReleaseToolLib = Join-Path $PSScriptRoot "lib"
. (Join-Path $script:ReleaseToolLib "common.ps1")
. (Join-Path $script:ReleaseToolLib "terminal-menu.ps1")
. (Join-Path $script:ReleaseToolLib "menu.ps1")
. (Join-Path $script:ReleaseToolLib "verification.ps1")
. (Join-Path $script:ReleaseToolLib "workflow.ps1")
. (Join-Path $script:ReleaseToolLib "artifacts.ps1")

$interactiveSelection = -not $PSBoundParameters.ContainsKey("Action")
if ($interactiveSelection) {
    $selectedAction = Select-ReleaseAction
    if (-not $selectedAction) {
        Write-Host "Release operation cancelled."
        return
    }
    $Action = $selectedAction
}

if (-not $Version) {
    $Version = Read-ReleaseVersion -RepositoryRoot $script:RepositoryRoot
}

$script:Tag = "v$Version"
if ($interactiveSelection) {
    if ($Action -eq "Tag") {
        $ConfirmTag = $true
    } elseif ($Action -eq "Publish") {
        $ConfirmPublish = $true
    } elseif ($Action -eq "Remove") {
        $ConfirmRemoveDraft = $true
    }
}

Push-Location $script:RepositoryRoot
try {
    foreach ($command in @("git", "node")) {
        Require-Command $command
    }
    Assert-Repository

    $resumeAction = $null
    while ($true) {
        try {
            switch ($Action) {
                "Verify" { Invoke-QualityGate }
                "Tag" { New-ReleaseTag }
                "Watch" { Watch-ReleaseWorkflow }
                "Retry" { Retry-ReleaseProcess }
                "Inspect" { Inspect-DraftRelease }
                "Publish" { Publish-Release }
                "Remove" { Remove-DraftReleaseAndTag }
            }
            if ($resumeAction) {
                $Action = $resumeAction
                $resumeAction = $null
                continue
            }
            break
        } catch {
            if (-not $interactiveSelection) {
                throw
            }
            $failureAction = Select-ReleaseFailureAction -Action $Action -Message $_.Exception.Message
            if ($failureAction -eq "RetryOperation") {
                continue
            }
            if ($failureAction -eq "RetryWorkflow") {
                $resumeAction = $Action
                $Action = "Retry"
                continue
            }
            Write-Warning "Release operation stopped. Completed steps were left unchanged."
            break
        }
    }
} finally {
    Pop-Location
    [Console]::InputEncoding = $previousConsoleInputEncoding
    [Console]::OutputEncoding = $previousConsoleOutputEncoding
    $OutputEncoding = $previousOutputEncoding
}

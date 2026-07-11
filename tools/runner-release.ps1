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
    [ValidateSet("Verify", "Tag", "Watch", "Inspect", "Publish")]
    [string]$Action,

    [ValidatePattern('^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$')]
    [string]$Version,

    [ValidateNotNullOrEmpty()]
    [string]$ReleaseBranch = "master",

    [string]$DownloadDirectory,
    [switch]$ConfirmTag,
    [switch]$ConfirmPublish
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

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

$interactiveSelection = -not $Action
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
    }
}

Push-Location $script:RepositoryRoot
try {
    foreach ($command in @("git", "node")) {
        Require-Command $command
    }
    Assert-Repository

    switch ($Action) {
        "Verify" { Invoke-QualityGate }
        "Tag" { New-ReleaseTag }
        "Watch" { Watch-ReleaseWorkflow }
        "Inspect" { Inspect-DraftRelease }
        "Publish" { Publish-Release }
    }
} finally {
    Pop-Location
}

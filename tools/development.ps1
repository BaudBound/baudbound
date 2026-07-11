<#
.SYNOPSIS
Launches BaudBound development applications and checks from an interactive menu.

.EXAMPLE
./tools/development.ps1

.EXAMPLE
./tools/development.ps1 -Action Desktop

.EXAMPLE
./tools/development.ps1 -Action Checks
#>
[CmdletBinding()]
param(
    [ValidateSet("Desktop", "DesktopUi", "Editor", "Service", "Status", "Install", "Checks", "Tests", "EditorE2E", "Schemas", "Build")]
    [string]$Action
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$toolLib = Join-Path $PSScriptRoot "lib"
. (Join-Path $toolLib "terminal-menu.ps1")
. (Join-Path $toolLib "development-menu.ps1")
. (Join-Path $toolLib "development-tasks.ps1")

Push-Location $repositoryRoot
try {
    if ($Action) {
        Invoke-DevelopmentTask -Task $Action
        return
    }

    while ($true) {
        $selectedAction = Select-DevelopmentAction
        if (-not $selectedAction) {
            Write-Host "Development helper closed."
            return
        }

        Invoke-DevelopmentTask -Task $selectedAction
        Wait-ForDevelopmentMenu
    }
} finally {
    Pop-Location
}

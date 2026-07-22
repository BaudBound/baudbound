<#
.SYNOPSIS
Launches BaudBound development applications and checks from an interactive menu.

.EXAMPLE
./tools/development.ps1

.EXAMPLE
./tools/development.ps1 -Action Desktop

.EXAMPLE
./tools/development.ps1 -Action Checks

.EXAMPLE
./tools/development.ps1 -Action RunnerBuild -Platform Both
#>
[CmdletBinding()]
param(
    [ValidateSet("Desktop", "DesktopUi", "Service", "Status", "Install", "Checks", "Tests", "Build", "RunnerBuild")]
    [string]$Action,

    [ValidateSet("Both", "Linux", "Windows")]
    [string]$Platform
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$previousConsoleInputEncoding = [Console]::InputEncoding
$previousConsoleOutputEncoding = [Console]::OutputEncoding
$previousOutputEncoding = $OutputEncoding
$utf8Encoding = [Text.UTF8Encoding]::new($false)
[Console]::InputEncoding = $utf8Encoding
[Console]::OutputEncoding = $utf8Encoding
$OutputEncoding = $utf8Encoding

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$toolLib = Join-Path $PSScriptRoot "lib"
. (Join-Path $toolLib "terminal-menu.ps1")
. (Join-Path $toolLib "development-menu.ps1")
. (Join-Path $toolLib "development-tasks.ps1")
. (Join-Path $toolLib "development-runner-build.ps1")

Push-Location $repositoryRoot
try {
    if ($PSBoundParameters.ContainsKey("Platform") -and $Action -ne "RunnerBuild") {
        throw "-Platform can only be used with -Action RunnerBuild."
    }

    if ($Action) {
        if ($Action -eq "RunnerBuild" -and -not $Platform) {
            $Platform = Select-RunnerBuildPlatform
            if (-not $Platform) {
                Write-Host "Runner build cancelled."
                return
            }
        }
        if ($Action -eq "RunnerBuild") {
            Invoke-DevelopmentTask -Task $Action -RunnerBuildPlatform $Platform
        } else {
            Invoke-DevelopmentTask -Task $Action
        }
        return
    }

    while ($true) {
        $selectedAction = Select-DevelopmentAction
        if (-not $selectedAction) {
            Write-Host "Development helper closed."
            return
        }

        $selectedPlatform = $null
        if ($selectedAction -eq "RunnerBuild") {
            $selectedPlatform = Select-RunnerBuildPlatform
            if (-not $selectedPlatform) {
                continue
            }
        }

        try {
            if ($selectedAction -eq "RunnerBuild") {
                Invoke-DevelopmentTask -Task $selectedAction -RunnerBuildPlatform $selectedPlatform
            } else {
                Invoke-DevelopmentTask -Task $selectedAction
            }
        } catch {
            Write-Host ""
            Write-Host "Development task failed" -ForegroundColor Red
            Write-Host $_.Exception.Message -ForegroundColor Red
        }
        Wait-ForDevelopmentMenu
    }
} finally {
    Pop-Location
    [Console]::InputEncoding = $previousConsoleInputEncoding
    [Console]::OutputEncoding = $previousConsoleOutputEncoding
    $OutputEncoding = $previousOutputEncoding
}

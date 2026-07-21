function Invoke-DevelopmentCommand {
    param(
        [Parameter(Mandatory)][string]$Command,
        [Parameter()][string[]]$Arguments = @(),
        [Parameter()][string]$WorkingDirectory
    )
    if (-not (Get-Command $Command -ErrorAction SilentlyContinue)) {
        throw "Required command '$Command' was not found in PATH."
    }
    Write-Host "> $Command $($Arguments -join ' ')" -ForegroundColor DarkGray
    if ($WorkingDirectory) {
        Push-Location $WorkingDirectory
    }
    try {
        & $Command @Arguments
        if ($LASTEXITCODE -ne 0) {
            throw "Command '$Command $($Arguments -join ' ')' failed with exit code $LASTEXITCODE."
        }
    } finally {
        if ($WorkingDirectory) {
            Pop-Location
        }
    }
}

function Stop-DevelopmentDesktopInstance {
    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
        return
    }

    $expectedExecutable = [IO.Path]::GetFullPath(
        (Join-Path (Get-Location).Path "target\debug\baudbound.exe")
    )
    $workspaceProcesses = @(
        Get-Process -Name "baudbound" -ErrorAction SilentlyContinue | Where-Object {
            try {
                $_.Path -and [IO.Path]::GetFullPath($_.Path).Equals(
                    $expectedExecutable,
                    [StringComparison]::OrdinalIgnoreCase
                )
            } catch {
                $false
            }
        }
    )

    foreach ($process in $workspaceProcesses) {
        Write-Host "Stopping stale desktop development instance (PID $($process.Id))..." -ForegroundColor Yellow
        $null = $process.CloseMainWindow()
        try {
            Wait-Process -Id $process.Id -Timeout 3 -ErrorAction Stop
        } catch {
            if (Get-Process -Id $process.Id -ErrorAction SilentlyContinue) {
                Stop-Process -Id $process.Id -Force -ErrorAction Stop
                Wait-Process -Id $process.Id -Timeout 5 -ErrorAction SilentlyContinue
            }
        }
    }

    if (Test-Path -LiteralPath $expectedExecutable) {
        $deadline = [DateTime]::UtcNow.AddSeconds(5)
        $unlocked = $false
        while ([DateTime]::UtcNow -lt $deadline) {
            try {
                $stream = [IO.File]::Open(
                    $expectedExecutable,
                    [IO.FileMode]::Open,
                    [IO.FileAccess]::ReadWrite,
                    [IO.FileShare]::None
                )
                $stream.Dispose()
                $unlocked = $true
                break
            } catch [IO.IOException] {
                Start-Sleep -Milliseconds 100
            } catch [UnauthorizedAccessException] {
                Start-Sleep -Milliseconds 100
            }
        }
        if (-not $unlocked) {
            throw "The desktop development executable is still in use: $expectedExecutable"
        }
    }

    $uiRoot = [IO.Path]::GetFullPath(
        (Join-Path (Get-Location).Path "apps\baudbound\ui")
    )
    $listeners = @(Get-NetTCPConnection -LocalPort 1420 -State Listen -ErrorAction SilentlyContinue)
    foreach ($listener in $listeners) {
        $owner = Get-CimInstance Win32_Process -Filter "ProcessId=$($listener.OwningProcess)"
        $isWorkspaceVite = $owner.Name -eq "node.exe" -and
            $owner.CommandLine -and
            $owner.CommandLine.IndexOf($uiRoot, [StringComparison]::OrdinalIgnoreCase) -ge 0 -and
            $owner.CommandLine.IndexOf("vite", [StringComparison]::OrdinalIgnoreCase) -ge 0
        if (-not $isWorkspaceVite) {
            throw "Port 1420 is used by an unrelated process (PID $($listener.OwningProcess))."
        }
        Write-Host "Stopping stale desktop UI development server (PID $($owner.ProcessId))..." -ForegroundColor Yellow
        Stop-Process -Id $owner.ProcessId -Force -ErrorAction Stop
    }

    $serverDeadline = [DateTime]::UtcNow.AddSeconds(5)
    while (
        (Get-NetTCPConnection -LocalPort 1420 -State Listen -ErrorAction SilentlyContinue) -and
        [DateTime]::UtcNow -lt $serverDeadline
    ) {
        Start-Sleep -Milliseconds 100
    }
    if (Get-NetTCPConnection -LocalPort 1420 -State Listen -ErrorAction SilentlyContinue) {
        throw "The desktop UI development port 1420 is still in use."
    }
}

function Invoke-DevelopmentTask {
    param(
        [Parameter(Mandatory)][string]$Task,
        [ValidateSet("Both", "Linux", "Windows")]
        [string]$RunnerBuildPlatform
    )

    if ($RunnerBuildPlatform -and $Task -ne "RunnerBuild") {
        throw "RunnerBuildPlatform can only be used with the RunnerBuild task."
    }

    switch ($Task) {
        "Desktop" {
            Stop-DevelopmentDesktopInstance
            Invoke-DevelopmentCommand "node" @("ui/node_modules/@tauri-apps/cli/tauri.js", "dev") -WorkingDirectory "apps/baudbound"
        }
        "DesktopUi" {
            Invoke-DevelopmentCommand "pnpm" @("--dir", "apps/baudbound/ui", "dev")
        }
        "Editor" {
            Invoke-DevelopmentCommand "pnpm" @("--dir", "apps/editor", "dev")
        }
        "Service" {
            Invoke-DevelopmentCommand "cargo" @("run", "-p", "baudbound", "--", "serve")
        }
        "Status" {
            Invoke-DevelopmentCommand "cargo" @("run", "-p", "baudbound", "--", "status")
        }
        "Install" {
            Invoke-DevelopmentCommand "pnpm" @("--dir", "apps/baudbound/ui", "install", "--frozen-lockfile")
            Invoke-DevelopmentCommand "pnpm" @("--dir", "apps/editor", "install", "--frozen-lockfile")
        }
        "Checks" {
            Invoke-DevelopmentCommand "cargo" @("fmt", "--all", "--", "--check")
            Invoke-DevelopmentCommand "cargo" @("clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings")
            Invoke-DevelopmentCommand "pnpm" @("--dir", "apps/editor", "lint")
            Invoke-DevelopmentCommand "pnpm" @("--dir", "apps/editor", "typecheck")
            Invoke-DevelopmentCommand "pnpm" @("--dir", "apps/editor", "schemas:check")
            Invoke-DevelopmentCommand "pnpm" @("--dir", "apps/baudbound/ui", "typecheck")
        }
        "Tests" {
            Invoke-DevelopmentCommand "cargo" @("test", "--workspace", "--locked")
            Invoke-DevelopmentCommand "pnpm" @("--dir", "apps/editor", "test")
            Invoke-DevelopmentCommand "pnpm" @("--dir", "apps/baudbound/ui", "test")
        }
        "EditorE2E" {
            Invoke-DevelopmentCommand "pnpm" @("--dir", "apps/editor", "e2e")
        }
        "Schemas" {
            Invoke-DevelopmentCommand "pnpm" @("--dir", "apps/editor", "schemas:generate")
            Invoke-DevelopmentCommand "pnpm" @("--dir", "apps/editor", "schemas:check")
        }
        "Build" {
            Invoke-DevelopmentCommand "pnpm" @("--dir", "apps/baudbound/ui", "build")
            Invoke-DevelopmentCommand "cargo" @("build", "-p", "baudbound", "--locked")
            Invoke-DevelopmentCommand "pnpm" @("--dir", "apps/editor", "build")
        }
        "RunnerBuild" {
            if (-not $RunnerBuildPlatform) {
                throw "RunnerBuild requires a platform. Choose Both, Linux, or Windows."
            }
            Invoke-LocalRunnerBuild -Platform $RunnerBuildPlatform
        }
        default {
            throw "Unknown development task: $Task"
        }
    }
}

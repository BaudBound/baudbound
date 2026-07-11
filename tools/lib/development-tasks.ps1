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

function Invoke-DevelopmentTask {
    param([Parameter(Mandatory)][string]$Task)

    switch ($Task) {
        "Desktop" {
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
    }
}

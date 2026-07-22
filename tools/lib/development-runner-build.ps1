function Invoke-LocalRunnerBuild {
    param(
        [Parameter(Mandatory)]
        [ValidateSet("Both", "Linux", "Windows")]
        [string]$Platform
    )

    $isWindows = [Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
        [Runtime.InteropServices.OSPlatform]::Windows
    )
    $isLinux = [Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
        [Runtime.InteropServices.OSPlatform]::Linux
    )

    if ($Platform -in @("Windows", "Both") -and -not $isWindows) {
        throw "Windows runner packages require a Windows host. Run the helper on Windows or choose Linux."
    }
    if ($Platform -in @("Linux", "Both") -and -not ($isLinux -or $isWindows)) {
        throw "Linux runner packages require Linux or Docker Desktop with Linux containers on Windows."
    }

    Write-Host ""
    Write-Host "Local builds are unsigned test packages and must not be published as release artifacts." -ForegroundColor Yellow

    if ($Platform -in @("Windows", "Both")) {
        Invoke-LocalWindowsRunnerBuild
    }
    if ($Platform -in @("Linux", "Both")) {
        if ($isLinux) {
            Invoke-LocalLinuxRunnerBuild
        } else {
            Invoke-DockerLinuxRunnerBuild
        }
    }
}

function Invoke-LocalWindowsRunnerBuild {
    Initialize-NativeRunnerBuild

    Write-Host ""
    Write-Host "Building the Windows NSIS installer..." -ForegroundColor Cyan
    Invoke-TauriLocalBuild -Bundles "nsis" -TargetDirectory "target/local-build/windows"
    Write-LocalRunnerArtifacts -Path "target/local-build/windows/release/bundle/nsis" -Label "Windows"
}

function Invoke-LocalLinuxRunnerBuild {
    Initialize-NativeRunnerBuild
    foreach ($commandName in @("dpkg-deb", "rpm")) {
        Assert-LocalRunnerBuildCommand $commandName
    }

    Write-Host ""
    Write-Host "Building the Linux AppImage, Debian package, and RPM package..." -ForegroundColor Cyan
    Invoke-TauriLocalBuild -Bundles "appimage,deb,rpm" -TargetDirectory "target/local-build/linux"
    $versionTag = Get-LocalRunnerVersionTag
    Invoke-DevelopmentCommand "node" @(
        "apps/baudbound/scripts/verify-linux-packages-cli.mjs",
        "target/local-build/linux/release/bundle",
        $versionTag
    )
    Write-LocalLinuxRunnerArtifacts
}

function Invoke-DockerLinuxRunnerBuild {
    Assert-LocalRunnerBuildCommand "docker"
    Assert-DockerLinuxEngine

    $image = "baudbound-linux-builder:local"
    $dockerfile = (Resolve-Path (Join-Path $PSScriptRoot "../docker/runner-linux.Dockerfile")).Path
    $dockerContext = Split-Path -Parent $dockerfile
    Write-Host ""
    Write-Host "Preparing the local Linux build container..." -ForegroundColor Cyan
    Invoke-DevelopmentCommand "docker" @(
        "build",
        "--tag", $image,
        "--file", $dockerfile,
        $dockerContext
    )

    $repositoryRoot = (Get-Location).Path
    $versionTag = Get-LocalRunnerVersionTag
    $artifactDirectory = Join-Path $repositoryRoot "target/local-build/linux/artifacts"
    New-Item -ItemType Directory -Path $artifactDirectory -Force | Out-Null
    Write-Host ""
    Write-Host "Building the Linux AppImage, Debian package, and RPM package in Docker..." -ForegroundColor Cyan
    Invoke-DevelopmentCommand "docker" @(
        "run", "--rm",
        "--mount", "type=bind,source=$repositoryRoot,target=/workspace",
        "--mount", "type=bind,source=$artifactDirectory,target=/artifacts",
        "--mount", "type=volume,source=baudbound-linux-cargo-registry,target=/root/.cargo/registry",
        "--mount", "type=volume,source=baudbound-linux-cargo-git,target=/root/.cargo/git",
        "--mount", "type=volume,source=baudbound-linux-pnpm-store,target=/workspace/.pnpm-store",
        "--mount", "type=volume,source=baudbound-linux-ui-modules,target=/workspace/apps/baudbound/ui/node_modules",
        "--mount", "type=volume,source=baudbound-linux-target,target=/workspace-target",
        "--mount", "type=tmpfs,target=/workspace/apps/baudbound/gen",
        "--env", "CARGO_TARGET_DIR=/workspace-target",
        "--workdir", "/workspace",
        $image,
        "bash", "-lc",
        "pnpm --dir apps/baudbound/ui install --frozen-lockfile && cd apps/baudbound && node ui/node_modules/@tauri-apps/cli/tauri.js build --bundles appimage,deb,rpm --ci --no-sign --config tauri.local-build.conf.json && rm -f /artifacts/*.AppImage /artifacts/*.deb /artifacts/*.rpm && cp /workspace-target/release/bundle/appimage/*.AppImage /workspace-target/release/bundle/deb/*.deb /workspace-target/release/bundle/rpm/*.rpm /artifacts/ && cd /workspace && node apps/baudbound/scripts/verify-linux-packages-cli.mjs /artifacts $versionTag"
    )
    Write-LocalRunnerArtifacts -Path "target/local-build/linux/artifacts" -Label "Linux"
}

function Get-LocalRunnerVersionTag {
    $configurationPath = Join-Path (Get-Location).Path "apps/baudbound/tauri.conf.json"
    $configuration = Get-Content -LiteralPath $configurationPath -Raw | ConvertFrom-Json
    $version = [string]$configuration.version
    if ($version -notmatch '^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$') {
        throw "Runner version '$version' is not a supported release version."
    }
    return "v$version"
}

function Write-LocalLinuxRunnerArtifacts {
    $bundleRoot = "target/local-build/linux/release/bundle"
    foreach ($bundle in @("appimage", "deb", "rpm")) {
        Write-LocalRunnerArtifacts -Path (Join-Path $bundleRoot $bundle) -Label "Linux $bundle"
    }
}

function Invoke-TauriLocalBuild {
    param(
        [Parameter(Mandatory)][string]$Bundles,
        [Parameter(Mandatory)][string]$TargetDirectory
    )

    $previousTargetDirectory = [Environment]::GetEnvironmentVariable("CARGO_TARGET_DIR", "Process")
    try {
        $env:CARGO_TARGET_DIR = Join-Path (Get-Location).Path $TargetDirectory
        Invoke-DevelopmentCommand "node" @(
            "ui/node_modules/@tauri-apps/cli/tauri.js",
            "build",
            "--bundles", $Bundles,
            "--ci",
            "--no-sign",
            "--config", "tauri.local-build.conf.json"
        ) -WorkingDirectory "apps/baudbound"
    } finally {
        if ($null -eq $previousTargetDirectory) {
            Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
        } else {
            $env:CARGO_TARGET_DIR = $previousTargetDirectory
        }
    }
}

function Initialize-NativeRunnerBuild {
    Assert-LocalRunnerBuildCommand "node"
    Assert-LocalRunnerBuildCommand "pnpm"
    Assert-LocalRunnerBuildCommand "cargo"
    Invoke-DevelopmentCommand "pnpm" @(
        "--dir", "apps/baudbound/ui",
        "install", "--frozen-lockfile"
    )
}

function Assert-DockerLinuxEngine {
    $operatingSystemType = (& docker info --format "{{.OSType}}" 2>$null | Out-String).Trim()
    if ($LASTEXITCODE -ne 0) {
        throw "Docker is installed but is not running. Start Docker Desktop and try again."
    }
    if ($operatingSystemType -ne "linux") {
        throw "Docker is not using Linux containers. Switch Docker Desktop to Linux containers and try again."
    }
}

function Assert-LocalRunnerBuildCommand {
    param([Parameter(Mandatory)][string]$Name)

    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Required command '$Name' was not found in PATH."
    }
}

function Write-LocalRunnerArtifacts {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string]$Label
    )

    $resolvedPath = Resolve-Path -LiteralPath $Path -ErrorAction SilentlyContinue
    if (-not $resolvedPath) {
        throw "$Label build completed without producing the expected artifact directory: $Path"
    }
    $artifacts = @(Get-ChildItem -LiteralPath $resolvedPath.Path -File)
    if ($artifacts.Count -eq 0) {
        throw "$Label build completed without producing an artifact in: $($resolvedPath.Path)"
    }

    Write-Host ""
    Write-Host "$Label build completed:" -ForegroundColor Green
    foreach ($artifact in $artifacts) {
        Write-Host "  $($artifact.FullName)"
    }
}

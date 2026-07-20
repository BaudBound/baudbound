function Invoke-QualityGate {
    foreach ($command in @("cargo", "git", "node", "pnpm")) {
        Require-Command $command
    }
    Assert-ReleaseVersion

    Write-Step "Installing exact locked JavaScript dependencies"
    Invoke-External "pnpm" @("--dir", "apps/baudbound/ui", "install", "--frozen-lockfile")
    Invoke-External "pnpm" @("--dir", "apps/editor", "install", "--frozen-lockfile")

    Write-Step "Checking Rust formatting and lint rules"
    Invoke-External "cargo" @("fmt", "--all", "--", "--check")
    Invoke-External "cargo" @("clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings")

    Write-Step "Running Rust workspace tests"
    Invoke-External "cargo" @("test", "--workspace", "--locked")

    Write-Step "Testing release artifact contracts"
    $artifactTests = @(
        Get-ChildItem "apps/baudbound/scripts" -Filter "*.test.mjs" |
            Sort-Object Name |
            ForEach-Object FullName
    )
    Invoke-External "node" (@("--test") + $artifactTests)

    Write-Step "Verifying editor schemas and contracts"
    Invoke-External "pnpm" @("--dir", "apps/editor", "schemas:check")
    Invoke-External "pnpm" @("--dir", "apps/editor", "test")

    Write-Step "Testing and building the desktop UI"
    Invoke-External "pnpm" @("--dir", "apps/baudbound/ui", "test")
    Invoke-External "pnpm" @("--dir", "apps/baudbound/ui", "build")

    Write-Step "Checking the pending Git diff"
    Invoke-External "git" @("diff", "--check")
    Write-Host "`nLocal release gate passed for $script:Tag." -ForegroundColor Green
    Write-Host "Commit and push the release commit. The Tag operation will require Runner CI for that exact commit."
}

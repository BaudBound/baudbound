function Write-Step {
    param([Parameter(Mandatory)][string]$Message)
    Write-Host "`n==> $Message" -ForegroundColor Cyan
}

function Require-Command {
    param([Parameter(Mandatory)][string]$Name)
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Required command '$Name' was not found in PATH."
    }
}

function Invoke-External {
    param(
        [Parameter(Mandatory)][string]$Command,
        [Parameter()][string[]]$Arguments = @()
    )
    Write-Host "  $Command $($Arguments -join ' ')" -ForegroundColor DarkGray
    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command '$Command $($Arguments -join ' ')' failed with exit code $LASTEXITCODE."
    }
}

function Invoke-ExternalCapture {
    param(
        [Parameter(Mandatory)][string]$Command,
        [Parameter()][string[]]$Arguments = @()
    )
    $output = & $Command @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command '$Command $($Arguments -join ' ')' failed with exit code $LASTEXITCODE."
    }
    return ($output -join [Environment]::NewLine).Trim()
}

function Assert-Repository {
    $topLevel = Invoke-ExternalCapture "git" @("rev-parse", "--show-toplevel")
    $resolvedTopLevel = (Resolve-Path $topLevel).Path
    if ($resolvedTopLevel -ne $script:RepositoryRoot) {
        throw "Resolved repository '$resolvedTopLevel', expected '$script:RepositoryRoot'."
    }
}

function Assert-CleanWorktree {
    $status = Invoke-ExternalCapture "git" @("status", "--porcelain")
    if ($status) {
        throw "The worktree is not clean. Commit or remove unrelated changes first.`n$status"
    }
}

function Assert-ReleaseBranch {
    $branch = Invoke-ExternalCapture "git" @("branch", "--show-current")
    if ($branch -ne $ReleaseBranch) {
        throw "Releases require branch '$ReleaseBranch'; the current branch is '$branch'."
    }
}

function Assert-ReleaseVersion {
    Write-Step "Verifying release version $script:Tag"
    Invoke-External "node" @("apps/baudbound/scripts/verify-release-version.mjs", $script:Tag)
}

function Get-WorkflowRun {
    param(
        [Parameter(Mandatory)][string]$Workflow,
        [Parameter(Mandatory)][string]$Commit
    )
    $json = Invoke-ExternalCapture "gh" @(
        "run", "list", "--workflow", $Workflow, "--commit", $Commit,
        "--limit", "10", "--json", "databaseId,status,conclusion,headSha,createdAt,url"
    )
    $runs = @($json | ConvertFrom-Json)
    if ($runs.Count -eq 0) {
        throw "No '$Workflow' run was found for commit $Commit."
    }
    return $runs[0]
}

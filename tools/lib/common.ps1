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
    $recentOutput = [Collections.Generic.Queue[string]]::new()
    $previousErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = "Continue"
        & $Command @Arguments 2>&1 | ForEach-Object {
            $line = $_.ToString()
            Write-Host $line
            $recentOutput.Enqueue($line)
            if ($recentOutput.Count -gt 20) {
                [void]$recentOutput.Dequeue()
            }
        }
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    if ($exitCode -ne 0) {
        $details = if ($recentOutput.Count -gt 0) {
            "`n" + ($recentOutput.ToArray() -join [Environment]::NewLine)
        } else {
            ""
        }
        throw "Command '$Command' failed with exit code $exitCode.$details"
    }
}

function Invoke-ExternalInteractive {
    param(
        [Parameter(Mandatory)][string]$Command,
        [Parameter()][string[]]$Arguments = @()
    )
    Write-Host "  $Command $($Arguments -join ' ')" -ForegroundColor DarkGray
    $previousErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = "Continue"
        & $Command @Arguments
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    if ($exitCode -ne 0) {
        throw "Command '$Command $($Arguments -join ' ')' failed with exit code $exitCode."
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
    $runs = Get-WorkflowRuns -Workflow $Workflow -Commit $Commit -Limit 10
    if ($runs.Count -eq 0) {
        throw "No '$Workflow' run was found for commit $Commit."
    }
    return $runs[0]
}

function Get-WorkflowRuns {
    param(
        [Parameter(Mandatory)][string]$Workflow,
        [Parameter(Mandatory)][string]$Commit,
        [ValidateRange(1, 100)][int]$Limit = 20
    )
    $json = Invoke-ExternalCapture "gh" @(
        "run", "list", "--workflow", $Workflow, "--commit", $Commit,
        "--limit", [string]$Limit,
        "--json", "databaseId,status,conclusion,headBranch,headSha,createdAt,url"
    )
    if (-not $json) {
        return @()
    }
    return @($json | ConvertFrom-Json)
}

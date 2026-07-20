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

function Remove-AnsiEscapeSequence {
    param([Parameter(Mandatory)][AllowEmptyString()][string]$Text)

    return [Text.RegularExpressions.Regex]::Replace(
        $Text,
        "\x1B(?:\[[0-?]*[ -/]*[@-~]|\][^\x07]*(?:\x07|\x1B\\))",
        ""
    )
}

function Get-NativeColorEnvironment {
    return [PSCustomObject]@{
        ForceColor = $env:FORCE_COLOR
        CliColorForce = $env:CLICOLOR_FORCE
        CargoTermColor = $env:CARGO_TERM_COLOR
        NoColor = $env:NO_COLOR
    }
}

function Enable-NativeColorEnvironment {
    Remove-Item Env:NO_COLOR -ErrorAction SilentlyContinue
    $env:FORCE_COLOR = "1"
    $env:CLICOLOR_FORCE = "1"
    $env:CARGO_TERM_COLOR = "always"
}

function Restore-NativeColorEnvironment {
    param([Parameter(Mandatory)]$Previous)

    $env:FORCE_COLOR = $Previous.ForceColor
    $env:CLICOLOR_FORCE = $Previous.CliColorForce
    $env:CARGO_TERM_COLOR = $Previous.CargoTermColor
    $env:NO_COLOR = $Previous.NoColor
}

function Invoke-External {
    param(
        [Parameter(Mandatory)][string]$Command,
        [Parameter()][string[]]$Arguments = @()
    )
    Write-Host "  $Command $($Arguments -join ' ')" -ForegroundColor DarkGray
    $recentOutput = [Collections.Generic.Queue[string]]::new()
    $previousErrorActionPreference = $ErrorActionPreference
    $previousColorEnvironment = Get-NativeColorEnvironment
    try {
        $ErrorActionPreference = "Continue"
        Enable-NativeColorEnvironment
        & $Command @Arguments 2>&1 | ForEach-Object {
            $line = $_.ToString()
            Write-Host $line
            $recentOutput.Enqueue((Remove-AnsiEscapeSequence -Text $line))
            if ($recentOutput.Count -gt 20) {
                [void]$recentOutput.Dequeue()
            }
        }
        $exitCode = $LASTEXITCODE
    } finally {
        Restore-NativeColorEnvironment -Previous $previousColorEnvironment
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
    $previousColorEnvironment = Get-NativeColorEnvironment
    try {
        $ErrorActionPreference = "Continue"
        Enable-NativeColorEnvironment
        & $Command @Arguments
        $exitCode = $LASTEXITCODE
    } finally {
        Restore-NativeColorEnvironment -Previous $previousColorEnvironment
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

function Assert-GitHubRepository {
    $repository = Invoke-ExternalCapture "gh" @(
        "repo", "view", "--json", "nameWithOwner", "--jq", ".nameWithOwner"
    )
    if ($repository -ne $script:Repository) {
        throw "GitHub CLI resolved repository '$repository', expected '$script:Repository'."
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
    $run = Find-WorkflowRun -Workflow $Workflow -Commit $Commit
    if (-not $run) {
        throw "No '$Workflow' run was found for commit $Commit."
    }
    return $run
}

function Find-WorkflowRun {
    param(
        [Parameter(Mandatory)][string]$Workflow,
        [Parameter(Mandatory)][string]$Commit
    )

    $runs = @(Get-WorkflowRuns -Workflow $Workflow -Commit $Commit -Limit 20)
    if ($runs.Count -eq 0) {
        return $null
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
        "--json", "databaseId,status,conclusion,event,headBranch,headSha,createdAt,url"
    )
    if (-not $json) {
        return @()
    }
    return @($json | ConvertFrom-Json)
}

function Wait-WorkflowRun {
    param(
        [Parameter(Mandatory)][string]$Workflow,
        [Parameter(Mandatory)][string]$Commit,
        [scriptblock]$Predicate = { param($Run) $true },
        [ValidateRange(1, 120)][int]$MaxAttempts = 20,
        [ValidateRange(0, 30)][int]$PollIntervalSeconds = 3,
        [switch]$AllowMissing
    )

    for ($attempt = 1; $attempt -le $MaxAttempts; $attempt++) {
        $runs = @(Get-WorkflowRuns -Workflow $Workflow -Commit $Commit -Limit 20)
        $matchingRuns = @($runs | Where-Object { & $Predicate $_ })
        if ($matchingRuns.Count -gt 0) {
            return $matchingRuns[0]
        }
        if ($attempt -lt $MaxAttempts) {
            Start-Sleep -Seconds $PollIntervalSeconds
        }
    }
    if ($AllowMissing) {
        return $null
    }
    $elapsedSeconds = ($MaxAttempts - 1) * $PollIntervalSeconds
    throw "No '$Workflow' run appeared for commit $Commit within $elapsedSeconds seconds."
}

function Get-WorkflowRunById {
    param([Parameter(Mandatory)][long]$RunId)

    $json = Invoke-ExternalCapture "gh" @(
        "run", "view", [string]$RunId,
        "--json", "databaseId,status,conclusion,event,headBranch,headSha,createdAt,url"
    )
    return $json | ConvertFrom-Json
}

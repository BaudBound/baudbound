function New-ReleaseTag {
    if (-not $ConfirmTag) {
        throw "Tag creation requires -ConfirmTag to prevent accidental release workflow runs."
    }
    Require-Command "gh"
    Assert-CleanWorktree
    Assert-ReleaseBranch
    Assert-ReleaseVersion

    $localCommit = Invoke-ExternalCapture "git" @("rev-parse", "HEAD")
    Assert-RemoteReleaseCommit -Commit $localCommit

    $remoteTagCommit = Get-RemoteTagCommit
    if ($remoteTagCommit) {
        if ($remoteTagCommit -ne $localCommit) {
            throw "Remote tag '$script:Tag' points to $remoteTagCommit instead of release commit $localCommit. Tags are never moved automatically."
        }
        Write-Host "`nRemote tag $script:Tag already points to the expected release commit. No push is needed." -ForegroundColor Green
        return
    }

    $localTagExists = [bool](Invoke-ExternalCapture "git" @("tag", "--list", $script:Tag))
    if ($localTagExists) {
        $tagCommit = Invoke-ExternalCapture "git" @("rev-list", "-n", "1", $script:Tag)
        if ($tagCommit -ne $localCommit) {
            throw "Local tag '$script:Tag' points to $tagCommit instead of release commit $localCommit."
        }
        Write-Step "Reusing local tag $script:Tag after an incomplete push"
    }

    Ensure-RunnerCiPassed -Commit $localCommit | Out-Null
    Assert-RemoteReleaseCommit -Commit $localCommit

    if ($script:ReleaseCmdlet.ShouldProcess("origin/$script:Tag", "Create and push the annotated release tag")) {
        if (-not $localTagExists) {
            Invoke-External "git" @("tag", "-a", $script:Tag, "-m", "BaudBound $script:Tag")
        }
        try {
            Invoke-External "git" @("push", "origin", $script:Tag)
        } catch {
            Write-Warning "The local tag exists but was not pushed. Inspect it before deleting it."
            throw
        }
        Write-Host "`nPushed $script:Tag. The Runner Release workflow is starting." -ForegroundColor Green
    }
}

function Assert-RemoteReleaseCommit {
    param([Parameter(Mandatory)][string]$Commit)

    Write-Step "Verifying release commit matches origin/$ReleaseBranch"
    Invoke-External "git" @("fetch", "--quiet", "origin", $ReleaseBranch)
    $remoteCommit = Invoke-ExternalCapture "git" @("rev-parse", "origin/$ReleaseBranch")
    if ($Commit -ne $remoteCommit) {
        throw "Release commit $Commit does not match origin/$ReleaseBranch $remoteCommit. Pull the current branch before continuing."
    }
}

function Watch-ReleaseWorkflow {
    Require-Command "gh"
    $commit = Resolve-RemoteReleaseTagCommit
    Write-Step "Watching the Runner Release workflow"
    $run = Wait-WorkflowRun `
        -Workflow $script:ReleaseWorkflow `
        -Commit $commit `
        -Predicate { param($Candidate) $Candidate.headBranch -eq $script:Tag }
    Watch-WorkflowRun -Run $run -Name "Runner Release workflow" | Out-Null
    Write-Host "`nRelease workflow passed. Inspect the draft before publishing." -ForegroundColor Green
}

function Get-WorkflowFailureSummary {
    param([Parameter(Mandatory)][long]$RunId)

    $json = Invoke-ExternalCapture "gh" @(
        "run", "view", [string]$RunId, "--json", "jobs"
    )
    $details = $json | ConvertFrom-Json
    $failures = [Collections.Generic.List[string]]::new()
    foreach ($job in @($details.jobs | Where-Object { $_.conclusion -eq "failure" })) {
        $failedSteps = @(
            $job.steps |
                Where-Object { $_.conclusion -eq "failure" } |
                ForEach-Object name
        )
        if ($failedSteps.Count -gt 0) {
            $failures.Add("$($job.name): $($failedSteps -join ', ')")
        } else {
            $failures.Add($job.name)
        }
    }
    if ($failures.Count -eq 0) {
        return "No failed job details were reported by GitHub."
    }
    return $failures -join " | "
}

function Watch-WorkflowRun {
    param(
        [Parameter(Mandatory)]$Run,
        [Parameter(Mandatory)][string]$Name
    )

    try {
        Invoke-ExternalInteractive "gh" @(
            "run", "watch", [string]$Run.databaseId, "--exit-status"
        )
    } catch {
        $current = Get-WorkflowRunById -RunId $Run.databaseId
        if ($current.status -eq "completed" -and $current.conclusion -ne "success") {
            $summary = Get-WorkflowFailureSummary -RunId $Run.databaseId
            throw "$Name failed (conclusion=$($current.conclusion)). Failed job or step: $summary. $($current.url)"
        }
        throw
    }

    $current = Get-WorkflowRunById -RunId $Run.databaseId
    if ($current.status -ne "completed" -or $current.conclusion -ne "success") {
        throw "$Name ended unexpectedly (status=$($current.status), conclusion=$($current.conclusion)). $($current.url)"
    }
    return $current
}

function Start-RunnerCiWorkflow {
    param([Parameter(Mandatory)][string]$Commit)

    Assert-RemoteReleaseCommit -Commit $Commit
    Write-Step "Starting Runner CI for release commit $Commit"
    Invoke-External "gh" @("workflow", "run", $script:CiWorkflow, "--ref", $ReleaseBranch)
    return Wait-WorkflowRun `
        -Workflow $script:CiWorkflow `
        -Commit $Commit `
        -Predicate { param($Candidate) $Candidate.event -eq "workflow_dispatch" }
}

function Ensure-RunnerCiPassed {
    param([Parameter(Mandatory)][string]$Commit)

    Write-Step "Checking Runner CI for $Commit"
    $run = Wait-WorkflowRun `
        -Workflow $script:CiWorkflow `
        -Commit $Commit `
        -MaxAttempts 5 `
        -PollIntervalSeconds 3 `
        -AllowMissing
    if (-not $run) {
        Write-Host "  No exact Runner CI run exists because GitHub may have skipped the push workflow."
        $run = Start-RunnerCiWorkflow -Commit $Commit
    }

    if ($run.status -ne "completed") {
        Write-Step "Runner CI is $($run.status). Watching run $($run.databaseId)"
        $run = Watch-WorkflowRun -Run $run -Name "Runner CI workflow"
    }
    if ($run.conclusion -ne "success") {
        throw "Runner CI failed (conclusion=$($run.conclusion)). Retry run $($run.databaseId) before tagging. $($run.url)"
    }
    Write-Host "`nRunner CI passed for the exact release commit." -ForegroundColor Green
    return $run
}

function Resolve-ReleaseTagCommit {
    $localTag = Invoke-ExternalCapture "git" @("tag", "--list", $script:Tag)
    $localCommit = if ($localTag) {
        Invoke-ExternalCapture "git" @("rev-list", "-n", "1", $script:Tag)
    } else {
        $null
    }
    $remoteCommit = Get-RemoteTagCommit

    if ($localCommit -and $remoteCommit -and $localCommit -ne $remoteCommit) {
        throw "Local tag '$script:Tag' points to $localCommit but origin points to $remoteCommit. Resolve the tag mismatch manually."
    }
    if ($localCommit) {
        return $localCommit
    }
    if ($remoteCommit) {
        Write-Step "Fetching $script:Tag from origin"
        Invoke-External "git" @(
            "fetch", "--quiet", "origin", "refs/tags/$script:Tag`:refs/tags/$script:Tag"
        )
        return Invoke-ExternalCapture "git" @("rev-list", "-n", "1", $script:Tag)
    }
    throw "Tag '$script:Tag' does not exist locally or on origin."
}

function Get-RemoteTagCommit {
    $peeled = Invoke-ExternalCapture "git" @(
        "ls-remote", "--tags", "origin", "refs/tags/$script:Tag^{}"
    )
    if ($peeled) {
        return ($peeled -split "\s+", 2)[0]
    }
    $direct = Invoke-ExternalCapture "git" @(
        "ls-remote", "--tags", "origin", "refs/tags/$script:Tag"
    )
    if ($direct) {
        return ($direct -split "\s+", 2)[0]
    }
    return $null
}

function Resolve-RemoteReleaseTagCommit {
    if (-not (Get-RemoteTagCommit)) {
        throw "Tag '$script:Tag' has not been pushed to origin, so no Runner Release workflow can exist. Run the Tag operation first."
    }
    return Resolve-ReleaseTagCommit
}

function Get-ReleaseWorkflowRun {
    $commit = Resolve-RemoteReleaseTagCommit
    return Wait-WorkflowRun `
        -Workflow $script:ReleaseWorkflow `
        -Commit $commit `
        -Predicate { param($Candidate) $Candidate.headBranch -eq $script:Tag }
}

function Assert-ReleaseWorkflowPassed {
    $run = Get-ReleaseWorkflowRun
    if ($run.status -ne "completed") {
        throw "Runner Release is still $($run.status). Watch run $($run.databaseId) before inspecting artifacts. $($run.url)"
    }
    if ($run.conclusion -ne "success") {
        throw "Runner Release failed (conclusion=$($run.conclusion)). Retry run $($run.databaseId) before inspecting artifacts. $($run.url)"
    }
    return $run
}

function Retry-WorkflowRun {
    param(
        [Parameter(Mandatory)]$Run,
        [Parameter(Mandatory)][string]$Name
    )
    $run = $Run
    if ($run.status -ne "completed") {
        Write-Step "$Name is $($run.status). Watching the existing run"
        Watch-WorkflowRun -Run $run -Name $Name | Out-Null
        Write-Host "`n$Name passed." -ForegroundColor Green
        return
    }
    if ($run.conclusion -eq "success") {
        Write-Host "`n$Name already passed. No retry is needed." -ForegroundColor Green
        return
    }

    Write-Step "Retrying $Name run $($run.databaseId)"
    if ($run.conclusion -eq "failure") {
        Invoke-External "gh" @("run", "rerun", [string]$run.databaseId, "--failed")
    } else {
        Invoke-External "gh" @("run", "rerun", [string]$run.databaseId)
    }
    Watch-WorkflowRun -Run $run -Name $Name | Out-Null
    Write-Host "`n$Name passed after retry." -ForegroundColor Green
}

function Retry-ReleaseProcess {
    Require-Command "gh"
    $remoteTagExists = [bool](Get-RemoteTagCommit)
    if ($remoteTagExists) {
        $run = Get-ReleaseWorkflowRun
        Retry-WorkflowRun -Run $run -Name "Runner Release workflow"
        return
    }

    Assert-ReleaseBranch
    $commit = Invoke-ExternalCapture "git" @("rev-parse", "HEAD")
    $run = Wait-WorkflowRun `
        -Workflow $script:CiWorkflow `
        -Commit $commit `
        -MaxAttempts 5 `
        -PollIntervalSeconds 3 `
        -AllowMissing
    if (-not $run) {
        $run = Start-RunnerCiWorkflow -Commit $commit
    }
    Retry-WorkflowRun -Run $run -Name "Runner CI workflow"
}

function Get-GitHubRelease {
    $json = Invoke-ExternalCapture "gh" @(
        "release", "list", "--limit", "1000",
        "--json", "tagName,name,isDraft,isPrerelease"
    )
    if (-not $json) {
        return $null
    }
    $matches = @(
        ($json | ConvertFrom-Json) | Where-Object { $_.tagName -eq $script:Tag }
    )
    if ($matches.Count -eq 0) {
        return $null
    }
    if ($matches.Count -gt 1) {
        throw "GitHub returned more than one release named '$script:Tag'."
    }
    return $matches[0]
}

function Wait-WorkflowCancellation {
    param([Parameter(Mandatory)][long]$RunId)

    for ($attempt = 1; $attempt -le 30; $attempt++) {
        $json = Invoke-ExternalCapture "gh" @("run", "view", [string]$RunId, "--json", "status")
        $run = $json | ConvertFrom-Json
        if ($run.status -eq "completed") {
            return
        }
        Start-Sleep -Seconds 2
    }
    throw "Workflow run $RunId did not stop within 60 seconds. The draft and tag were not removed."
}

function Stop-ActiveReleaseWorkflows {
    $localTagExists = [bool](Invoke-ExternalCapture "git" @("tag", "--list", $script:Tag))
    $remoteTagExists = [bool](Get-RemoteTagCommit)
    if (-not $localTagExists -and -not $remoteTagExists) {
        return
    }
    $commit = Resolve-ReleaseTagCommit
    $runs = @(Get-WorkflowRuns -Workflow $script:ReleaseWorkflow -Commit $commit)
    $activeRuns = @(
        $runs | Where-Object {
            $_.headBranch -eq $script:Tag -and $_.status -ne "completed"
        }
    )
    foreach ($run in $activeRuns) {
        Write-Step "Cancelling release workflow run $($run.databaseId)"
        Invoke-External "gh" @("run", "cancel", [string]$run.databaseId)
    }
    foreach ($run in $activeRuns) {
        Wait-WorkflowCancellation -RunId $run.databaseId
    }
}

function Remove-DraftReleaseAndTag {
    if (-not $ConfirmRemoveDraft) {
        throw "Draft removal requires -ConfirmRemoveDraft to prevent deleting the wrong release."
    }
    Require-Command "gh"

    $release = Get-GitHubRelease
    if ($release -and -not $release.isDraft) {
        throw "Release '$script:Tag' is published. Published releases and tags cannot be removed by this helper."
    }
    $localTagExists = [bool](Invoke-ExternalCapture "git" @("tag", "--list", $script:Tag))
    $remoteTagExists = [bool](Get-RemoteTagCommit)
    if (-not $release -and -not $localTagExists -and -not $remoteTagExists) {
        throw "No draft release or tag named '$script:Tag' exists."
    }
    if ($script:ReleaseCmdlet.ShouldProcess(
        "draft release and tag $script:Tag",
        "Cancel active release workflows and permanently remove the unpublished release"
    )) {
        if (-not $localTagExists -and $remoteTagExists) {
            Resolve-ReleaseTagCommit | Out-Null
            $localTagExists = $true
        }
        Stop-ActiveReleaseWorkflows

        if ($release) {
            Write-Step "Removing draft release and remote tag $script:Tag"
            Invoke-External "gh" @("release", "delete", $script:Tag, "--cleanup-tag", "--yes")
        } elseif ($remoteTagExists) {
            Write-Step "Removing remote tag $script:Tag"
            Invoke-External "git" @("push", "origin", "--delete", $script:Tag)
        }
        if (Get-GitHubRelease) {
            throw "GitHub still reports release '$script:Tag' after deletion. The local tag was preserved."
        }
        if (Get-RemoteTagCommit) {
            throw "Origin still reports tag '$script:Tag' after deletion. The local tag was preserved."
        }
        if (Invoke-ExternalCapture "git" @("tag", "--list", $script:Tag)) {
            Write-Step "Removing local tag $script:Tag"
            Invoke-External "git" @("tag", "--delete", $script:Tag)
        }
        Write-Host "`nRemoved unpublished release state for $script:Tag. The source commit remains on its branch." -ForegroundColor Green
    }
}

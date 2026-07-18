function New-ReleaseTag {
    if (-not $ConfirmTag) {
        throw "Tag creation requires -ConfirmTag to prevent accidental release workflow runs."
    }
    Require-Command "gh"
    Assert-CleanWorktree
    Assert-ReleaseBranch
    Assert-ReleaseVersion

    Write-Step "Verifying local $ReleaseBranch matches origin/$ReleaseBranch"
    Invoke-External "git" @("fetch", "--quiet", "origin", $ReleaseBranch)
    $localCommit = Invoke-ExternalCapture "git" @("rev-parse", "HEAD")
    $remoteCommit = Invoke-ExternalCapture "git" @("rev-parse", "origin/$ReleaseBranch")
    if ($localCommit -ne $remoteCommit) {
        throw "Local HEAD $localCommit does not match origin/$ReleaseBranch $remoteCommit."
    }

    if (Invoke-ExternalCapture "git" @("ls-remote", "--tags", "origin", "refs/tags/$script:Tag")) {
        throw "Remote tag '$script:Tag' already exists. Use Watch or Retry instead of moving it."
    }

    $localTagExists = [bool](Invoke-ExternalCapture "git" @("tag", "--list", $script:Tag))
    if ($localTagExists) {
        $tagCommit = Invoke-ExternalCapture "git" @("rev-list", "-n", "1", $script:Tag)
        if ($tagCommit -ne $localCommit) {
            throw "Local tag '$script:Tag' points to $tagCommit instead of release commit $localCommit."
        }
        Write-Step "Reusing local tag $script:Tag after an incomplete push"
    }

    Write-Step "Checking Runner CI for $localCommit"
    $run = Get-WorkflowRun -Workflow $script:CiWorkflow -Commit $localCommit
    if ($run.status -ne "completed" -or $run.conclusion -ne "success") {
        throw "Runner CI has not passed (status=$($run.status), conclusion=$($run.conclusion)). $($run.url)"
    }

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

function Watch-ReleaseWorkflow {
    Require-Command "gh"
    $commit = Invoke-ExternalCapture "git" @("rev-list", "-n", "1", $script:Tag)
    if (-not $commit) {
        throw "Tag '$script:Tag' is unavailable locally. Run 'git fetch --tags'."
    }
    Write-Step "Watching the Runner Release workflow"
    $run = Get-WorkflowRun -Workflow $script:ReleaseWorkflow -Commit $commit
    Invoke-External "gh" @("run", "watch", [string]$run.databaseId, "--exit-status")
    Write-Host "`nRelease workflow passed. Inspect the draft before publishing." -ForegroundColor Green
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

function Get-ReleaseWorkflowRun {
    $commit = Resolve-ReleaseTagCommit
    $runs = @(Get-WorkflowRuns -Workflow $script:ReleaseWorkflow -Commit $commit)
    $tagRuns = @($runs | Where-Object { $_.headBranch -eq $script:Tag })
    if ($tagRuns.Count -eq 0) {
        throw "No Runner Release workflow was found for $script:Tag at commit $commit."
    }
    return $tagRuns[0]
}

function Retry-WorkflowRun {
    param(
        [Parameter(Mandatory)]$Run,
        [Parameter(Mandatory)][string]$Name
    )
    $run = $Run
    if ($run.status -ne "completed") {
        Write-Step "$Name is $($run.status). Watching the existing run"
        Invoke-External "gh" @("run", "watch", [string]$run.databaseId, "--exit-status")
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
    Invoke-External "gh" @("run", "watch", [string]$run.databaseId, "--exit-status")
    Write-Host "`n$Name passed after retry." -ForegroundColor Green
}

function Retry-ReleaseProcess {
    Require-Command "gh"
    $localTagExists = [bool](Invoke-ExternalCapture "git" @("tag", "--list", $script:Tag))
    $remoteTagExists = [bool](Get-RemoteTagCommit)
    if ($localTagExists -or $remoteTagExists) {
        $run = Get-ReleaseWorkflowRun
        Retry-WorkflowRun -Run $run -Name "Runner Release workflow"
        return
    }

    Assert-ReleaseBranch
    $commit = Invoke-ExternalCapture "git" @("rev-parse", "HEAD")
    $run = Get-WorkflowRun -Workflow $script:CiWorkflow -Commit $commit
    Retry-WorkflowRun -Run $run -Name "Runner CI workflow"
}

function Get-DraftRelease {
    $arguments = @(
        "release", "view", $script:Tag,
        "--json", "tagName,name,isDraft,isPrerelease,url"
    )
    Write-Host "  gh $($arguments -join ' ')" -ForegroundColor DarkGray
    $previousErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = "Continue"
        $output = & gh @arguments 2>&1
        $exitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    $outputText = ($output -join [Environment]::NewLine).Trim()
    if ($exitCode -ne 0) {
        if ($outputText -eq "release not found") {
            return $null
        }
        throw "Command 'gh $($arguments -join ' ')' failed with exit code $exitCode.`n$outputText"
    }
    if (-not $outputText) {
        return $null
    }
    return $outputText | ConvertFrom-Json
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

    $release = Get-DraftRelease
    if ($release -and -not $release.isDraft) {
        throw "Release '$script:Tag' is published. Published releases and tags cannot be removed by this helper."
    }
    $localTagExists = [bool](Invoke-ExternalCapture "git" @("tag", "--list", $script:Tag))
    $remoteTagExists = [bool](Get-RemoteTagCommit)
    if (-not $release -and -not $localTagExists -and -not $remoteTagExists) {
        throw "No draft release or tag named '$script:Tag' exists."
    }
    if ($interactiveSelection) {
        $typedTag = Read-Host "Type '$script:Tag' to confirm permanent draft and tag removal"
        if ($typedTag -cne $script:Tag) {
            throw "Draft removal confirmation did not match '$script:Tag'."
        }
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
        if (Get-DraftRelease) {
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

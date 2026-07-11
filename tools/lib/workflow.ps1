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

    if (Invoke-ExternalCapture "git" @("tag", "--list", $script:Tag)) {
        throw "Local tag '$script:Tag' already exists. Release tags must never be moved."
    }
    if (Invoke-ExternalCapture "git" @("ls-remote", "--tags", "origin", "refs/tags/$script:Tag")) {
        throw "Remote tag '$script:Tag' already exists. Release tags must never be moved."
    }

    Write-Step "Checking Runner CI for $localCommit"
    $run = Get-WorkflowRun -Workflow $script:CiWorkflow -Commit $localCommit
    if ($run.status -ne "completed" -or $run.conclusion -ne "success") {
        throw "Runner CI has not passed (status=$($run.status), conclusion=$($run.conclusion)). $($run.url)"
    }

    if ($script:ReleaseCmdlet.ShouldProcess("origin/$script:Tag", "Create and push the annotated release tag")) {
        Invoke-External "git" @("tag", "-a", $script:Tag, "-m", "BaudBound $script:Tag")
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
    Invoke-External "gh" @("run", "watch", [string]$run.databaseId, "--compact", "--exit-status")
    Write-Host "`nRelease workflow passed. Inspect the draft before publishing." -ForegroundColor Green
}

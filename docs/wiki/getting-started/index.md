---
title: Getting Started
description: Build, simulate, export, approve, and run a first BaudBound workflow.
tags: [getting-started]
---
# Getting Started

This guide builds a safe first workflow that writes `Hello from BaudBound` to the runner log. It uses a **Manual** trigger and a **Log** action, works on every supported target runtime, and does not modify files or control the desktop.

## Before you begin

You need:

- A current web browser.
- Access to the [public BaudBound editor](https://editor.baudbound.app/).
- A Windows or Linux machine where you can install the runner.

You do not need an account. Install the runner by following [Installation and Updates](../runner/installation.md), then return here. The runner creates its configuration and storage automatically on first launch.

## 1. Create the project

1. Open the [BaudBound editor](https://editor.baudbound.app/).
2. Open **Project settings** from the top bar.
3. Set **Name** to `Hello BaudBound`.
4. Keep **Target runtime** set to **Generic Desktop**.
5. Save the settings.

The project name identifies the workflow in the editor and becomes the default package name. The target runtime limits the project to nodes that the intended runner can support.

## 2. Add and connect the nodes

1. Find **Manual** in the Triggers section of the node library and add it to the canvas.
2. Find **Log** in the Actions section and add it to the canvas.
3. Drag a connection from the Manual output handle to the Log input handle.
4. Select the Log node.
5. Set **Log level** to **Info**.
6. Replace **Message** with `Hello from BaudBound`.

The canvas should contain one path:

```text
Manual -> Log
```

The Log action writes to BaudBound's run log. It does not print into an unrelated terminal or create a text file.

## 3. Verify and simulate

1. Open the **Simulation** tab in the right inspector.
2. Find the Manual trigger and choose its trigger button.
3. Wait for verification and simulation to finish.
4. Open the **Simulation** output tab if it is not already visible.

A successful run shows a verification result with no blocking failures and a log entry containing:

```text
Hello from BaudBound
```

If verification fails, do not export yet. Confirm that both nodes are connected and the Log message is not empty. [Verification and Simulation](../editor/simulation.md) explains the full result categories.

## 4. Export the package

1. Choose **Export** in the top bar.
2. Review the package summary and verification results.
3. Continue only when export has no blocking failures.
4. Download the package.

Your browser downloads a file ending in `.bbs`. The exact filename is shown by the browser. Keep the file intact: the runner records a hash of the imported package, and later file changes invalidate that installed revision.

## 5. Import and inspect

### Desktop application

1. Open BaudBound.
2. Open **Scripts**.
3. Choose **Import package** and select the `.bbs` file.
4. Find the new script in the list.
5. Open its approval review and confirm that the package contains a Manual trigger and Log action with low risk.

The script row shows the installed name and identity. Use that displayed name for later CLI commands. It may differ from the downloaded filename.

### Command line

Open PowerShell on Windows or a terminal on Linux. Replace `PACKAGE` with the actual path to the downloaded `.bbs` file.

```text
baudbound script import "PACKAGE"
baudbound script list
```

The import command prints the installed script name and ID. In the remaining examples, replace `SCRIPT` with either value.

Inspect the package before approving it:

```text
baudbound script inspect SCRIPT
```

## 6. Approve and run

Approval accepts the exact imported package hash and its declared access. An updated package must be reviewed again.

In the desktop application, choose **Approve** from the approval review, then choose **Run** on the script row.

The equivalent CLI commands are:

```text
baudbound script approve SCRIPT
baudbound script run SCRIPT
```

Open **Runs** or **Logs** in the desktop application, or run:

```text
baudbound script logs --script SCRIPT --limit 5
```

The latest successful run should contain `Hello from BaudBound`.

## Stop and investigate when

Do not approve or run the package when:

- Editor verification reports a blocking failure.
- Import reports an integrity, schema, or compatibility error.
- The approval review contains nodes, permissions, capabilities, or risk you did not expect.
- The package came from someone you do not trust and you have not reviewed its behavior.

Start with [Runs, Logs, and Troubleshooting](../runner/runs-logs-troubleshooting.md) if the expected result does not appear.

## Next steps

- Follow [Tutorials](../tutorials/index.md) for schedules, webhooks, serial devices, file watching, and branches.
- Read [Visual Editor](../editor/index.md) for everyday editing workflows.
- Learn how data moves through a workflow in [Variables and Data](../editor/variables.md).
- Manage imported revisions with [Script Management](../runner/script-management.md).

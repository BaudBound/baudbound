---
title: BaudBound Documentation
description: Learn how to build, verify, and run local automations with BaudBound.
tags: [overview]
---
# BaudBound

BaudBound is a visual automation platform for Windows and Linux. You build a workflow by connecting nodes in the browser-based editor, export it as a `.bbs` package, and run it on your own machine with the native BaudBound runner.

BaudBound can automate tasks such as reacting to a schedule or file change, receiving a local webhook, communicating with a serial device, transforming data, calling an HTTP API, managing files and processes, and controlling supported desktop applications. Available actions depend on the selected target runtime and operating system.

## Choose where to start

| I want to... | Start here |
| --- | --- |
| Build and run my first workflow | [Getting Started](getting-started/index.md) |
| Learn with complete examples | [Tutorials](tutorials/index.md) |
| Understand BaudBound terminology | [Concepts and Glossary](concepts.md) |
| Learn the visual editor | [Visual Editor](editor/index.md) |
| Install or update the runner | [Installation and Updates](runner/installation.md) |
| Keep triggers running on a desktop | [Background Service and Triggers](runner/service-triggers.md) |
| Run BaudBound on a headless Linux machine | [Linux Background Service](runner/linux-background-service.md) |
| Host the editor on your own server | [Editor Self Hosting](self-hosting/index.md) |
| Contribute to BaudBound | [Developer Overview](developers/index.md) |

## How a workflow reaches the runner

1. **Build:** Add triggers, actions, and control-flow nodes in the editor.
2. **Verify:** Let the editor check graph structure, node configuration, variables, permissions, and target compatibility.
3. **Simulate:** Test the workflow with controlled input before it can affect a real machine.
4. **Export:** Download a portable `.bbs` package containing the graph, metadata, access declarations, optional editor data, and declared assets.
5. **Review:** Import the package into the runner and inspect its requested capabilities and risk.
6. **Approve and run:** Approve that exact package revision, then run it manually or activate its listener-based triggers.

The editor describes what a workflow should do. The runner independently validates that description and performs approved native actions locally. Editor simulation is useful for testing, but it is not a substitute for runner validation or approval.

## Local execution and explicit trust

The public editor does not need an account and does not execute production actions on the runner machine. Secrets are declared in the project but configured on the runner, so secret values do not need to be included in exported packages.

Every imported revision is checked for package integrity, schema validity, target-runtime compatibility, permissions, and capabilities. Approval applies to one exact package hash. Updating the package requires another review.

Read [Security Model](security/index.md) before approving workflows obtained from another person or exposing a listener to a network.

## Supported platforms

BaudBound supports Windows and Linux. Some nodes require a desktop session, and some native actions are available only on a narrower platform set. The editor prevents known-incompatible nodes for the selected [target runtime](editor/target-runtimes.md), and the runner checks compatibility again before execution.

## What BaudBound does not provide

- BaudBound is not a hosted cloud execution service. Workflows run on a runner you control.
- The public editor does not require or manage user accounts.
- A `.bbs` package is not trusted merely because it was created by the BaudBound editor.
- Unsupported operating systems and unsupported native actions are not silently emulated by the production runner.

Ready to try it? Continue with [Getting Started](getting-started/index.md).

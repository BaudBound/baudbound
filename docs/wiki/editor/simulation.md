---
title: Verification and Simulation
description: Verify graphs and test trigger paths safely in the editor.
tags: [editor, simulation]
---
# Verification and Simulation

Trigger buttons in the Simulation tab are always available. Activating one first verifies the project and then starts a simulation if no blocking errors remain. Use the Stop button to cancel an active simulation.

## Verification checks

Verification covers graph structure, reachability, node configuration, variables, conditions, target runtime compatibility, package metadata, secret declarations, and risk-bearing capabilities. Blocking errors prevent simulation and export. Warnings identify behavior that needs review but may still be valid.

## Runtime differences

Simulation exercises graph execution and data flow in the browser. It cannot reproduce every native runner operation. Desktop input, operating-system state, serial hardware, process behavior, filesystem permissions, listener lifetimes, and network policy must also be tested on the intended runner.

## Simulation data

The simulation panel lets you define trigger payloads and runtime values. For declared secrets, choose whether to use placeholders or enter actual values for a test. Actual secret values are kept only in the current editor session and are not written to project state, `editor.json`, or the exported package.

## Logs and variables

The output console records verification and execution events. Runtime variable snapshots appear in the Variables tab. At instant speed, execution advances without artificial node delays except behavior whose semantics require elapsed time.

Before release, simulate every trigger, branch, error path, loop termination, and expected output. Then validate and inspect the exported package as described in [Script Management](../runner/quick-start.md).

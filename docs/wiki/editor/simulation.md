---
title: Verification and Simulation
description: Understand editor verification rules, simulation controls, trigger payloads, side effects, and runner differences.
tags: [editor, verification, simulation]
---
# Verification and Simulation

Verification checks whether a project is internally valid. Simulation executes its graph in the browser with test input. Runner validation and approval happen later on the target machine. Passing one stage never skips the others.

## Compare the stages

| Stage | Runs where | Performs native production actions | Main purpose |
| --- | --- | --- | --- |
| **Verification** | Editor | No | Find graph, config, variable, asset, access, and target problems |
| **Simulation** | Editor browser | Mostly no; see side effects below | Exercise branches and data with controlled input |
| **Runner validation** | Native runner | No | Reparse package and recalculate integrity, security, target, and semantic contracts |
| **Runner execution** | Native runner | Yes, after checks and approval | Perform the approved automation |

Graph or project changes invalidate the previous editor verification state.

## Verification checks

The Verify command and export gate run these rule families:

| Check | What it validates |
| --- | --- |
| **Script metadata** | Required name and selected target runtime |
| **Secret references** | Unique declarations and no collision with writable variables |
| **Target runtime** | Every node supports the selected platform/session target |
| **Graph structure** | Runnable nodes exist and at most one Manual trigger is present |
| **Entry points** | At least one trigger and valid trigger configuration |
| **Connections** | Source/target nodes and named handles exist |
| **Permissions** | Medium, high, or dangerous access is surfaced for review |
| **Variables** | Writable names, references, calculations, node config, and graph-specific rules |
| **Assets** | Package paths, allowed files, duplicates, limits, and node references |
| **Serial devices** | Logical IDs, duplicate triggers, and write targets |
| **Export readiness** | Combined blocking conditions permit package creation |

Node definitions add their own config and graph rules. Examples include a waiting webhook requiring a reachable response node, Serial Write requiring a matching Serial Input logical ID, valid loop structure, and condition-row syntax.

## Failures, warnings, and passing checks

**Failed** checks block simulation or export when the graph cannot be executed safely or serialized consistently. Fix them before continuing.

**Warnings** require review but do not necessarily make the graph invalid. Medium-or-higher permissions are warnings because the editor cannot decide whether intended access is acceptable.

**Passed** means the tested contract succeeded with current editor information. It does not prove that paths, credentials, ports, hardware, desktop sessions, or external services will exist on the runner.

## Simulation controls

Open **Simulation** in the inspector. Each trigger has a card containing fields appropriate to that trigger and a button to fire it. Schedule triggers run automatically while the simulation session remains active.

| Control | Behavior |
| --- | --- |
| **Trigger button** | Verifies, starts the session, and fires that trigger payload |
| **Speed** | Delays steps at Slow, Normal, Fast, or Instant pacing |
| **Runtime override** | Forces a selected fallible node to succeed or fail for branch testing |
| **Stop** | Cancels the active run or waiting session |
| **Simulation output** | Shows verification, trigger, node, branch, side-effect, and failure traces |
| **Runtime data** | Shows current variable and node-output snapshots |

Only one trigger executes at a time. Stop an active run before firing another. Editing the graph cancels and resets the previous simulation state.

## Trigger payload examples

Payload fields are test input, not saved production events.

| Trigger | Useful simulation fields |
| --- | --- |
| Manual | Optional generic payload supplied by the card |
| Schedule | Fires automatically from configured interval |
| File Watch | `path`, `event` such as `modified` |
| Webhook | `method`, `path`, headers, query, raw `body`, JSON body |
| WebSocket | `path`, `message`, `connectionId`, headers, query, `remoteAddress` |
| Hotkey | `key` |
| Serial Input | `data`; output also calculates byte length and timestamp |
| Process Started | `processName`, `processId`, `executablePath`, `windowTitle` |
| Startup | `reason`, normally represented as runner startup |

Example webhook body:

```json
{
  "event": "simulation",
  "message": "hello"
}
```

Use the trigger node's runtime output browser to insert its real ID into downstream references.

## Secrets in simulation

Secret declarations appear in the output console's secret manager. Each declaration can use a placeholder or a value you enter for the current browser session.

Entered values are type-checked, supplied to simulation, redacted from simulation reports where applicable, and never written into the exported package. Reloading, importing, or clearing the session can remove them.

> A real secret used during simulation can still be sent by an HTTP Request node or exposed to any browser behavior the workflow invokes. Use test credentials by default and understand every path before entering production data.
{.is-warning}

## Side effects and fidelity

The browser performs a small controlled set of visible simulation effects:

- notification actions show editor toasts;
- message boxes show an editor dialog and return the selected button;
- Beep uses browser Web Audio;
- Play Sound can play a packaged audio asset; and
- HTTP Request uses the browser's real `fetch` API.

HTTP simulation can contact a real server. Browser CORS, forbidden headers, user-agent handling, cookies, TLS trust, and network policy differ from the native runner.

Other native actions are described and assigned simulated outputs without controlling the machine. This includes files, processes, shell, keyboard, mouse, clipboard, windows, physical serial ports, application opening, and WebSocket listener connections.

Simulation cannot prove:

- runner filesystem paths or account permissions;
- executable or process availability;
- desktop focus and session behavior;
- serial protocol or device identity;
- listener binding, firewall, proxy, or port ownership;
- package approval, runner policy, or encrypted secret storage; or
- native error timing and operating-system-specific output.

## Runtime overrides

Add an override for a fallible action to test its **success** or **failed** edge without reproducing the external failure. Failed overrides populate structured error output so downstream error handling can be exercised.

Remove overrides before considering the simulation representative. Overrides are editor test settings and are not production runner policy.

## Logs and variables

Simulation traces identify the node and branch being executed. Log actions also produce output entries at `debug`, `info`, `warn`, or `error` level.

The runtime-data view updates after each step with current variables and node outputs. Use it to confirm types and nested fields rather than relying only on formatted log text.

## Pre-export checklist

- [ ] Every verification check passes or has an understood warning.
- [ ] Every trigger has been tested with representative valid and invalid payloads.
- [ ] True, false, switch, loop, done, success, and failed branches behave as intended.
- [ ] Temporary runtime overrides are removed or accounted for.
- [ ] Secret simulation values use test credentials unless real access is explicitly intended.
- [ ] HTTP requests target safe test endpoints.
- [ ] Runtime data has the expected types and nested paths.
- [ ] Native behavior still requiring runner testing is listed for the operator.
- [ ] Export access review contains only expected permissions and capabilities.

Continue with [Projects, Assets, and Export](projects-assets-export.md) and [Variables and Data](variables.md).

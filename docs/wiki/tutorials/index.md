---
title: Tutorials
description: Complete BaudBound examples for schedules, webhooks, serial devices, files, and branching data.
tags: [tutorials, editor, runner]
---
# Tutorials

These tutorials start in the editor and finish on a runner. Complete [Getting Started](../getting-started/index.md) first so package import and approval are familiar.

Use a separate project for each tutorial. After building a graph, trigger it in the editor's **Simulation** tab before exporting. Simulation checks the graph with controlled input. The final runner test checks that the real listener or device works on the target machine.

## Scheduled log

This low-risk workflow writes a timestamped message once per minute while the background service is running.

**Target runtime:** Generic Desktop or a supported headless runtime  
**Nodes:** [Schedule](../editor/node-reference.md#triggers), [Log](../editor/node-reference.md#data-and-output)
**Runner requirement:** schedule triggers enabled and `baudbound serve` or the desktop background runner active

### Build

1. Add a **Schedule** trigger.
2. Set **Every** to `1` and **Unit** to **Minutes**.
3. Add a **Log** action and connect Schedule to Log.
4. Set **Log level** to **Info**.
5. Set **Message** to `Scheduled run at {{system_time}}`.

The graph is:

```text
Schedule -> Log
```

### Test and run

1. Open **Simulation**. Schedule triggers fire automatically while simulation remains active.
2. Confirm that the Simulation output contains `Scheduled run at` followed by a time.
3. Export, import, inspect, and approve the package.
4. Enable the installed script.
5. Start the desktop background runner or run `baudbound serve` in a terminal.
6. Wait at least one minute, then open **Runs** or **Logs**.

Expected result: one successful run and one timestamped log message for each due interval. If nothing runs, check the **Triggers** and **Service** views and confirm that schedules are enabled in [runner configuration](../runner/configuration.md).

### Cleanup

Disable or remove the script when the recurring run is no longer needed. Stop `baudbound serve` with `Ctrl+C` if you started it in a terminal.

## Local webhook with a response

This workflow accepts a local HTTP POST, logs a value from its JSON body, and returns JSON to the caller.

> Exposing a webhook beyond `127.0.0.1` changes its security boundary. Keep this tutorial local. Do not bind to `0.0.0.0` without authentication, firewall, proxy, and request-limit planning.
{.is-warning}

**Target runtime:** any supported runtime  
**Nodes:** [Webhook](../editor/node-reference.md#triggers), [Log](../editor/node-reference.md#data-and-output), [Webhook Response](../editor/node-reference.md#network-and-serial)
**Runner requirement:** webhook listener enabled on `127.0.0.1:43891`

### Build

1. Add a **Webhook** trigger.
2. Set **Method** to **POST** and **Hook name** to `tutorial`.
3. Enable **Wait for response node**.
4. Keep **Response timeout seconds** at `30`.
5. Add a **Log** action and connect Webhook to Log.
6. Insert the Webhook node's `json.message` output into the Log **Message** field. The editor creates a reference containing that node's real ID, similar to `{{NODE_ID.json.message}}`.
7. Add a **Webhook Response** action and connect Log to it.
8. Set **Status code** to `200`, **Content type** to `application/json`, and **Body** to `{ "received": true }`.

The graph is:

```text
Webhook -> Log -> Webhook Response
```

Do not type the literal placeholder `NODE_ID`. Use the variable browser so the reference points to the Webhook node in your project.

### Simulate

1. Open **Simulation** and locate the Webhook trigger.
2. Enter a JSON request body containing `{ "message": "hello" }`.
3. Trigger the simulation.
4. Confirm that the Log action resolves `hello` and the response trace reports status `200`.

### Configure and call the runner

1. In **Config**, enable webhooks and keep the bind address `127.0.0.1` and port `43891`.
2. Export, import, inspect, approve, and enable the script.
3. Copy the Webhook token shown after approval. If it was not saved, open **Security**, generate a replacement, and copy it immediately.
4. Start the background runner.
5. Set the token in the command and send the request from the same machine.

### Request {.tabset}

#### PowerShell

```powershell
$token = "PASTE_TOKEN_HERE"
Invoke-RestMethod `
  -Method Post `
  -Uri "http://127.0.0.1:43891/events/tutorial" `
  -Headers @{ "X-BaudBound-Token" = $token } `
  -ContentType "application/json" `
  -Body '{ "message": "hello" }'
```

#### Linux shell

```bash
TOKEN='PASTE_TOKEN_HERE'
curl --fail-with-body \
  --request POST \
  --header "X-BaudBound-Token: $TOKEN" \
  --header 'Content-Type: application/json' \
  --data '{ "message": "hello" }' \
  http://127.0.0.1:43891/events/tutorial
```

Expected response:

```json
{ "received": true }
```

Expected runner result: a successful run whose log contains `hello`. A `401` means the token header is missing. A `403` means the token is wrong. A `404` means no active route matches the method and hook name. A connection error usually means the listener is stopped, disabled, or using another address or port.

### Cleanup

Disable the tutorial script and stop its background runner if it was started only for this test. Keep the webhook listener disabled in configuration when no installed script needs it.

## Serial input and reply

This workflow receives text from a serial device and writes `ack` back to the same logical device.

> Serial actions access physical hardware and require explicit approval. Confirm device identity, voltage level, baud rate, and protocol from the hardware documentation before sending data.
{.is-warning}

**Target runtime:** a Windows or Linux runtime supported by the connected device  
**Nodes:** [Serial Input](../editor/node-reference.md#triggers), [Log](../editor/node-reference.md#data-and-output), [Serial Write](../editor/node-reference.md#network-and-serial)
**Runner requirement:** one serial mapping with logical device ID `tutorial-device`

### Build

1. Add a **Serial Input** trigger.
2. Set **Device id** to `tutorial-device`.
3. Add a **Log** action and connect Serial Input to Log.
4. Insert the Serial Input node's `data` output into the Log **Message** field.
5. Add a **Serial Write** action and connect Log to Serial Write.
6. Set its device to `tutorial-device`, data to `ack`, and line ending to the value required by the device. Use **None** when the protocol does not require LF or CRLF.

Both serial nodes must use the same logical ID. A logical ID is not a Windows `COM` name or Linux `/dev` path.

### Simulate

1. Open **Simulation** and enter `ping` as the Serial Input data.
2. Trigger the simulation.
3. Confirm that the Log action receives `ping` and Serial Write reports that it would send `ack`.

Simulation does not open the physical port.

### Map the physical device

1. Connect the device and open the runner's **Devices** view.
2. Choose **Scan** and identify the correct port by its manufacturer, product, vendor ID, product ID, and serial number where available.
3. Choose **Add**, enter `tutorial-device`, and configure the protocol settings required by the device.
4. Enable identity validation when stable USB identity fields are available.
5. Save the configuration and restart the background runner.
6. Re-scan and confirm that the logical device resolves to the intended hardware.

Detailed identity and port-rebinding behavior is documented in [Configuration and Serial Devices](../runner/configuration.md).

### Run

1. Export, import, inspect, approve, and enable the script.
2. Start the background runner.
3. Send `ping` from the physical device.
4. Confirm that the run log contains `ping` and the device receives `ack`.

If the trigger remains inactive, check **Devices** before reconnecting repeatedly. The runner refuses ambiguous identity matches instead of guessing between similar devices.

### Cleanup

Disable the script before disconnecting test hardware. Remove the `tutorial-device` mapping from Config when it is no longer used.

## Watch a file

This workflow starts when a test file changes and logs the event and path.

**Target runtime:** any supported runtime with access to the chosen path  
**Nodes:** [File Watch](../editor/node-reference.md#triggers), [Log](../editor/node-reference.md#data-and-output)
**Runner requirement:** file-watch triggers enabled and the configured path present before the listener starts

### Prepare a test path {.tabset}

#### Windows

Open PowerShell:

```powershell
New-Item -ItemType Directory -Force "$HOME\baudbound-tutorial"
Set-Content "$HOME\baudbound-tutorial\input.txt" "initial"
```

Use the full path printed by:

```powershell
(Resolve-Path "$HOME\baudbound-tutorial\input.txt").Path
```

#### Linux

Open a terminal:

```bash
mkdir -p "$HOME/baudbound-tutorial"
printf '%s\n' 'initial' > "$HOME/baudbound-tutorial/input.txt"
```

Use the full path printed by:

```bash
readlink -f "$HOME/baudbound-tutorial/input.txt"
```

### Build and simulate

1. Add a **File Watch** trigger.
2. Set **Path** to the full test-file path. Do not use `~` or a runtime variable.
3. Leave **Include subdirectories** disabled because the target is one file.
4. Add a **Log** action and connect File Watch to Log.
5. Insert the trigger's `event` and `path` outputs into a message such as `File event EVENT at PATH`, using the variable browser for both values.
6. In Simulation, provide a path and `modified` event, then confirm both appear in the log message.

### Run

1. Export, import, inspect, approve, and enable the script.
2. Start the background runner.
3. Change the file.

### Change the file {.tabset}

#### Windows

```powershell
Add-Content "$HOME\baudbound-tutorial\input.txt" "changed"
```

#### Linux

```bash
printf '%s\n' 'changed' >> "$HOME/baudbound-tutorial/input.txt"
```

Expected result: a successful run whose log reports the changed path and an event such as `modified`. Operating systems may report more than one low-level event for one application save.

### Cleanup {.tabset}

#### Windows

Disable the script, stop the test service if needed, then run:

```powershell
Remove-Item -Recurse -Force "$HOME\baudbound-tutorial"
```

#### Linux

Disable the script, stop the test service if needed, then run:

```bash
rm -rf "$HOME/baudbound-tutorial"
```

## Branch and persist a counter

This workflow increments a persistent counter and logs a different message after the third run.

**Target runtime:** any supported runtime  
**Nodes:** [Manual](../editor/node-reference.md#triggers), [Variable Operation](../editor/node-reference.md#data-and-output), [If/Else](../editor/node-reference.md#control-flow), [Log](../editor/node-reference.md#data-and-output)
**Runner requirement:** approval for persistent storage

### Build

1. Add a **Manual** trigger.
2. Add a **Variable Operation** node and connect Manual to it.
3. Set **Operation** to **Increment**, **Variable name** to `run_count`, **Scope** to **Persistent**, and **Amount** to `1`.
4. Add an **If / Else** node and connect Variable Operation to it.
5. Set its condition to `{{run_count}}` **greater than or equal** `3`.
6. Add two **Log** actions.
7. Connect the If/Else **true** output to a Log message `Run count reached {{run_count}}`.
8. Connect the **false** output to a Log message `Run count is {{run_count}}`.

The graph is:

```text
Manual -> Variable Operation -> If / Else -> true  -> Log reached
                                           -> false -> Log count
```

### Simulate and run

Simulation confirms branch wiring and typed comparison, but do not use it as proof that production persistence is configured. Trigger it enough times to observe both branches in the current simulation session.

Then:

1. Export and import the package.
2. Review the persistent-storage permission and medium risk before approving it.
3. Run the script three times from the runner.
4. Open its run logs.

Expected result: the first two runs follow the false branch, and the third follows the true branch with a count of `3`. Further runs continue increasing the stored value.

### Cleanup

Removing or reinstalling a script may affect its stored state according to runner storage rules. Do not use a destructive cleanup command merely to reset one value on a production runner. Use this tutorial only in a test runner home until the [storage and recovery documentation](../runner/runs-logs-troubleshooting.md) describes the intended operation for your release.

## Continue learning

- [Variables and Data](../editor/variables.md) explains output references and value types.
- [Node Reference](../editor/node-reference.md) lists all available building blocks.
- [Background Service and Triggers](../runner/service-triggers.md) explains listener lifecycle.
- [Runs, Logs, and Troubleshooting](../runner/runs-logs-troubleshooting.md) helps diagnose failed examples.

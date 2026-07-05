import type { Node } from "@xyflow/react";
import { createHeaderRow, type HeaderRow } from "@/data/nodes/definitions/rows";
import type { ScriptNodeData, SimulationTriggerPayload } from "@/lib/types";

export function createDefaultTriggerPayload(triggerNode: Node<ScriptNodeData>): SimulationTriggerPayload {
	switch (triggerNode.data.actionType) {
		case "trigger.serial_input":
			return { data: "simulation serial input" };
		case "trigger.file_watch":
			return {
				path: String(triggerNode.data.config.path ?? "/path/to/watch"),
				event: "modified",
			};
		case "trigger.webhook":
			return {
				method: String(triggerNode.data.config.method ?? "POST"),
				path: `/events/${String(triggerNode.data.config.hookName ?? "name")}`,
				body: '{\n  "event": "simulation"\n}',
			};
		case "trigger.websocket":
			return {
				path: String(triggerNode.data.config.path ?? "/events/messages"),
				connectionId: "simulated-connection",
				remoteAddress: "127.0.0.1",
				message: '{\n  "event": "simulation"\n}',
			};
		case "trigger.hotkey":
			return {};
		case "trigger.startup":
			return { reason: "runner_startup" };
		case "trigger.process_started":
			return {
				processName: String(triggerNode.data.config.target ?? "app.exe"),
				processId: "4244",
				executablePath: "",
				windowTitle: "",
			};
		default:
			return {};
	}
}

export function createDefaultWebhookHeaders(): HeaderRow[] {
	return [createHeaderRow("content-type", "application/json")];
}

export function createTriggerPayload(
	triggerNode: Node<ScriptNodeData>,
	payload: SimulationTriggerPayload,
	headers: HeaderRow[],
	query: HeaderRow[],
): SimulationTriggerPayload {
	if (triggerNode.data.actionType !== "trigger.webhook" && triggerNode.data.actionType !== "trigger.websocket") {
		return payload;
	}

	return {
		...payload,
		headers: rowsToRecord(headers),
		query: rowsToRecord(query),
	};
}

function rowsToRecord(rows: HeaderRow[]): Record<string, string> {
	return Object.fromEntries(rows.map((row) => [row.name.trim(), row.value]).filter(([name]) => name.length > 0));
}

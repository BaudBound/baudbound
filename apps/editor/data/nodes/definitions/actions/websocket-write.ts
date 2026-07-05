import { Send } from "lucide-react";
import type { JsonValue } from "@/lib/types";
import { defineNode } from "../../node-definition";
import { fallible } from "../runtime-outputs";
import { requiredConfig } from "../validators";

export const websocketWriteNode = defineNode({
	actionType: "action.websocket.write",
	capabilities: ["action.websocket"],
	configFields: [
		{
			key: "connectionId",
			label: "Connection id",
			type: "text",
			usesVariables: true,
			help: "Use the WebSocket Trigger connection_id output, for example {{node-id.connection_id}}.",
		},
		{ key: "message", label: "Message", type: "textarea", usesVariables: true },
	],
	defaultConfig: () => ({
		connectionId: "simulated-connection",
		message: '{ "ok": true }',
	}),
	description: "Send a message to an active WebSocket connection.",
	fallible: true,
	group: "actions",
	icon: Send,
	kind: "action",
	label: "WebSocket Write",
	permission: { name: "websocket_write", risk: "medium" },
	risk: "medium",
	runtimeOutputs: fallible([
		{
			name: "connection_id",
			type: "string",
			description: "WebSocket connection id targeted by the write.",
			example: "n-mr3zyt6f-24.connection_id",
		},
		{
			name: "message",
			type: "string",
			description: "Message sent to the connection.",
			example: "n-mr3zyt6f-24.message",
		},
		{
			name: "bytes",
			type: "number",
			description: "UTF-8 byte length of the sent message.",
			example: "n-mr3zyt6f-24.bytes",
		},
	]),
	runnerType: "websocket_write",
	validateConfig: (config) =>
		[
			requiredConfig(config, "connectionId", "WebSocket connection id"),
			requiredConfig(config, "message", "WebSocket message"),
		].filter(Boolean),
	validateGraph: ({ context, node }) =>
		context.nodes.some((otherNode) => otherNode.data.actionType === "trigger.websocket")
			? []
			: [`${node.id} writes to a WebSocket connection, but the script has no WebSocket Trigger.`],
	simulation: {
		createOutput: ({ api, context, node }) => {
			const connectionId = String(api.resolveTemplate(api.getConfigString(node, "connectionId"), context)).trim();
			const message = String(api.resolveTemplate(api.getConfigString(node, "message"), context));

			if (!connectionId) {
				const outputData: Record<string, JsonValue> = {
					error: api.createError(
						"WebSocket Write requires a connection id.",
						"WEBSOCKET_CONNECTION_ID_MISSING",
						"validation",
						{ node_id: node.id },
						false,
					),
				};

				return {
					failed: true,
					outputData,
				};
			}

			const outputData: Record<string, JsonValue> = {
				connection_id: connectionId,
				message,
				bytes: new TextEncoder().encode(message).length,
			};

			return {
				failed: false,
				outputData,
			};
		},
		describe: ({ api, context, failed, node }) => {
			const output = context.nodeOutputs[node.id] ?? {};
			if (failed) {
				const error =
					output.error && typeof output.error === "object" && !Array.isArray(output.error) ? output.error : {};
				return [
					{
						level: "error",
						message: `[Simulation] WebSocket Write (${node.id}) failed. ${String(error.message ?? "Message was not sent.")}`,
					},
				];
			}

			return [
				{
					level: "info",
					message: `[Simulation] WebSocket Write (${node.id}) sent ${String(output.bytes ?? "?")} bytes to connection ${api.formatValue(output.connection_id ?? "")}. Message: ${truncateMessage(String(output.message ?? ""))}`,
				},
			];
		},
	},
});

function truncateMessage(value: string) {
	return value.length > 180 ? `${value.slice(0, 179)}...` : value;
}

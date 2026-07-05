import { Radio } from "lucide-react";
import { defineNode } from "../../node-definition";
import { triggerPorts } from "../shared";
import { configString, requiredConfig } from "../validators";

export const websocketTriggerNode = defineNode({
	actionType: "trigger.websocket",
	capabilities: ["trigger.websocket"],
	configFields: [
		{
			key: "path",
			label: "Path",
			type: "text",
			help: "Runner-side WebSocket path, for example /events/messages. The runner decides host and port.",
		},
	],
	defaultConfig: () => ({ path: "/events/messages" }),
	description: "Start when a WebSocket message is received.",
	group: "triggers",
	icon: Radio,
	kind: "trigger",
	label: "WebSocket",
	ports: triggerPorts,
	permission: { name: "websocket_public_bind", risk: "high" },
	risk: "high",
	runtimeOutputs: [
		{
			name: "path",
			type: "string",
			description: "WebSocket path that received the message.",
			example: "n-mr3zyt6f-6.path",
		},
		{
			name: "connection_id",
			type: "string",
			description: "Runner-provided connection identifier.",
			example: "n-mr3zyt6f-6.connection_id",
		},
		{
			name: "headers",
			type: "http_headers",
			description: "WebSocket handshake headers when available.",
			example: 'n-mr3zyt6f-6.headers["sec-websocket-protocol"]',
		},
		{
			name: "query",
			type: "object",
			description: "Parsed WebSocket path query parameters.",
			example: "n-mr3zyt6f-6.query.token",
		},
		{ name: "message", type: "string", description: "Raw WebSocket message payload.", example: "n-mr3zyt6f-6.message" },
		{
			name: "json",
			type: "object",
			description: "Parsed JSON message when the payload is JSON.",
			example: "n-mr3zyt6f-6.json.event",
		},
		{
			name: "remote_address",
			type: "string",
			description: "Remote peer address when the runner exposes it.",
			example: "n-mr3zyt6f-6.remote_address",
		},
	],
	runnerType: "websocket",
	validateConfig: (config) => {
		const path = configString(config, "path").trim();
		return [
			requiredConfig(config, "path", "WebSocket path"),
			path && !path.startsWith("/") ? 'WebSocket path must start with "/".' : "",
		].filter(Boolean);
	},
	simulation: {
		createOutput: ({ api, context, node }) => {
			const message = context.triggerPayload.message || '{"event":"simulation"}';
			const json = api.parseJsonValue(message);

			return {
				failed: false,
				outputData: {
					path: context.triggerPayload.path || api.getConfigString(node, "path") || "/events/messages",
					connection_id: context.triggerPayload.connectionId || "simulated-connection",
					headers: context.triggerPayload.headers ?? {},
					query: context.triggerPayload.query ?? {},
					message,
					json: json ?? {},
					remote_address: context.triggerPayload.remoteAddress || "127.0.0.1",
				},
			};
		},
	},
});

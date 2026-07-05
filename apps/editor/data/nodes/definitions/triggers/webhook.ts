import { Globe } from "lucide-react";
import { defineNode } from "../../node-definition";
import { httpMethodOptions } from "../options";
import { triggerPorts } from "../shared";
import { requiredConfig } from "../validators";

export const webhookTriggerNode = defineNode({
	actionType: "trigger.webhook",
	capabilities: ["trigger.webhook"],
	configFields: [
		{ key: "method", label: "Method", type: "select", options: httpMethodOptions },
		{ key: "hookName", label: "Hook name", type: "text" },
	],
	defaultConfig: () => ({ method: "POST", hookName: "name" }),
	description: "Start from a local webhook.",
	group: "triggers",
	icon: Globe,
	kind: "trigger",
	label: "Webhook",
	ports: triggerPorts,
	permission: { name: "webhook_public_bind", risk: "high" },
	risk: "high",
	runtimeOutputs: [
		{
			name: "method",
			type: "string",
			description: "HTTP method used to call the webhook.",
			example: "n-mr3zyt6f-3.method",
		},
		{
			name: "path",
			type: "string",
			description: "Webhook path received by the runner.",
			example: "n-mr3zyt6f-3.path",
		},
		{
			name: "headers",
			type: "http_headers",
			description: "Webhook request headers.",
			example: 'n-mr3zyt6f-3.headers["content-type"]',
		},
		{
			name: "query",
			type: "object",
			description: "Parsed query string parameters.",
			example: "n-mr3zyt6f-3.query.token",
		},
		{
			name: "body",
			type: "string",
			description: "Raw request body.",
			example: "n-mr3zyt6f-3.body",
		},
		{
			name: "json",
			type: "object",
			description: "Parsed JSON body when the request body is JSON.",
			example: "n-mr3zyt6f-3.json.event",
		},
	],
	runnerType: "webhook",
	validateConfig: (config) => [requiredConfig(config, "hookName", "webhook hook name")].filter(Boolean),
	simulation: {
		createOutput: ({ api, context, node }) => {
			const body = context.triggerPayload.body || '{"event":"simulation"}';
			const json = api.parseJsonValue(body);

			return {
				failed: false,
				outputData: {
					method: context.triggerPayload.method || api.getConfigString(node, "method") || "POST",
					path: context.triggerPayload.path || `/events/${api.getConfigString(node, "hookName") || "name"}`,
					headers: context.triggerPayload.headers ?? { "content-type": "application/json" },
					query: context.triggerPayload.query ?? {},
					body,
					json: json ?? {},
				},
			};
		},
	},
});

import { Globe } from "lucide-react";
import type { JsonValue } from "@/lib/types";
import { defineNode } from "../../node-definition";
import { httpMethodOptions } from "../options";
import { configString, requiredConfig, staticPositiveNumberConfig } from "../validators";

export const webhookTriggerNode = defineNode({
	actionType: "trigger.webhook",
	capabilities: ["trigger.webhook"],
	configFields: [
		{ key: "method", label: "Method", type: "select", options: httpMethodOptions },
		{ key: "hookName", label: "Hook name", type: "text" },
		{ key: "waitForResponse", label: "Wait for response node", type: "switch", required: false },
		{
			key: "responseTimeoutSeconds",
			label: "Response timeout seconds",
			type: "number",
			required: false,
			numeric: {
				kind: "float",
				signed: false,
				minimum: "0",
				maximum: "86400",
				minimumInclusive: false,
				maximumInclusive: true,
			},
		},
		{
			key: "timeoutResponseStatus",
			label: "Fallback response status",
			type: "number",
			required: false,
			numeric: {
				kind: "integer",
				signed: false,
				minimum: "100",
				maximum: "599",
				minimumInclusive: true,
				maximumInclusive: true,
			},
		},
		{
			key: "timeoutResponseContentType",
			label: "Fallback content type",
			type: "text",
			usesVariables: true,
			required: false,
		},
		{
			key: "timeoutResponseBody",
			label: "Fallback response body",
			type: "textarea",
			usesVariables: true,
			required: false,
		},
	],
	defaultConfig: () => ({
		method: "POST",
		hookName: "name",
		waitForResponse: false,
		responseTimeoutSeconds: "30",
		timeoutResponseStatus: "200",
		timeoutResponseContentType: "application/json",
		timeoutResponseBody: '{ "ok": true }',
	}),
	description: "Start from a local webhook.",
	group: "triggers",
	icon: Globe,
	kind: "trigger",
	label: "Webhook",
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
		{
			name: "response",
			type: "object",
			description:
				"Webhook response state for this request, including whether the runner is waiting for a response node.",
			example: "n-mr3zyt6f-3.response.waiting",
		},
	],
	runnerType: "webhook",
	validateConfig: (config) =>
		[
			requiredConfig(config, "hookName", "webhook hook name"),
			staticPositiveNumberConfig(config, "responseTimeoutSeconds", "webhook response timeout"),
			validateHttpStatusConfig(config, "timeoutResponseStatus", "fallback response status"),
		].filter(Boolean),
	validateGraph: ({ context, node }) => {
		if (!isEnabled(node.data.config.waitForResponse)) {
			return [];
		}

		return canReachAction(node.id, "action.webhook_response", context.edges, context.nodes)
			? []
			: [`${node.id} waits for a webhook response, but no Webhook Response node is reachable from it.`];
	},
	simulation: {
		createOutput: ({ api, context, node }) => {
			const body = context.triggerPayload.body || '{"event":"simulation"}';
			const json = api.parseJsonValue(body);
			const waitForResponse = isEnabled(node.data.config.waitForResponse);
			const fallbackResponse = {
				body: String(api.resolveTemplate(api.getConfigString(node, "timeoutResponseBody"), context)),
				content_type:
					String(api.resolveTemplate(api.getConfigString(node, "timeoutResponseContentType"), context)).trim() ||
					"text/plain",
				headers: {},
				status_code: normalizeHttpStatus(api.getConfigString(node, "timeoutResponseStatus"), 200),
			};

			context.webhookResponse = {
				fallback: fallbackResponse,
				sent: !waitForResponse,
				triggerNodeId: node.id,
				waiting: waitForResponse,
			};

			return {
				failed: false,
				outputData: {
					method: context.triggerPayload.method || api.getConfigString(node, "method") || "POST",
					path: context.triggerPayload.path || `/events/${api.getConfigString(node, "hookName") || "name"}`,
					headers: context.triggerPayload.headers ?? { "content-type": "application/json" },
					query: context.triggerPayload.query ?? {},
					body,
					json: json ?? {},
					response: waitForResponse
						? createWaitingResponseState(
								normalizePositiveNumber(api.getConfigString(node, "responseTimeoutSeconds"), 30),
							)
						: createImmediateResponseState(fallbackResponse),
				},
			};
		},
		describe: ({ api, context, node }) => {
			const waitForResponse = isEnabled(node.data.config.waitForResponse);
			const hookName = api.getConfigString(node, "hookName") || "name";

			if (waitForResponse) {
				return [
					{
						level: "info",
						message: `[Simulation] Webhook (${node.id}) fired /events/${hookName} and is waiting up to ${normalizePositiveNumber(api.getConfigString(node, "responseTimeoutSeconds"), 30)}s for a Webhook Response node.`,
					},
				];
			}

			const response = context.webhookResponse?.fallback;
			return [
				{
					level: "info",
					message: `[Simulation] Webhook (${node.id}) fired /events/${hookName} and immediately responded ${response?.status_code ?? 200} ${response?.content_type ?? "text/plain"}.`,
				},
			];
		},
	},
});

function validateHttpStatusConfig(config: Record<string, JsonValue>, key: string, label: string) {
	const value = configString(config, key).trim();
	if (!value) {
		return `must define ${label}.`;
	}

	const status = Number(value);
	return Number.isInteger(status) && status >= 100 && status <= 599 ? "" : `${label} must be an HTTP status 100-599.`;
}

function normalizeHttpStatus(value: string, fallback: number) {
	const status = Number(value);
	return Number.isInteger(status) && status >= 100 && status <= 599 ? status : fallback;
}

function normalizePositiveNumber(value: string, fallback: number) {
	const number = Number(value);
	return Number.isFinite(number) && number > 0 ? number : fallback;
}

function createWaitingResponseState(timeoutSeconds: number): Record<string, JsonValue> {
	return {
		waiting: true,
		timeout_seconds: timeoutSeconds,
	};
}

function createImmediateResponseState(response: Record<string, JsonValue>): Record<string, JsonValue> {
	return {
		...response,
		sent: true,
		waiting: false,
	};
}

function isEnabled(value: JsonValue | undefined) {
	return value === true || value === "true";
}

function canReachAction(
	startNodeId: string,
	actionType: string,
	edges: { source: string; target: string }[],
	nodes: Array<{ id: string; data: { actionType: string } }>,
) {
	const nodesById = new Map(nodes.map((node) => [node.id, node]));
	const visited = new Set<string>();
	const queue = [startNodeId];

	while (queue.length > 0) {
		const nodeId = queue.shift();
		if (!nodeId || visited.has(nodeId)) {
			continue;
		}

		visited.add(nodeId);
		const node = nodesById.get(nodeId);
		if (node?.data.actionType === actionType) {
			return true;
		}

		for (const edge of edges) {
			if (edge.source === nodeId) {
				queue.push(edge.target);
			}
		}
	}

	return false;
}

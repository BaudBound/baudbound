import { Reply } from "lucide-react";
import type { JsonValue } from "@/lib/types";
import type { SimulationContext } from "@/utils/simulation-types";
import { defineNode, type NodeSimulationApi } from "../../node-definition";
import { createHeaderRow, isHeaderRow } from "../rows";
import { fallible } from "../runtime-outputs";
import { configString, requiredConfig } from "../validators";

export const webhookResponseNode = defineNode({
	actionType: "action.webhook_response",
	capabilities: ["action.webhook_response"],
	configFields: [
		{ key: "statusCode", label: "Status code", type: "number", usesVariables: true },
		{ key: "contentType", label: "Content type", type: "text", usesVariables: true },
		{ key: "body", label: "Body", type: "textarea", usesVariables: true },
	],
	defaultConfig: () => ({
		statusCode: "200",
		contentType: "application/json",
		headers: [createHeaderRow("Cache-Control", "no-store")],
		body: '{ "ok": true }',
	}),
	description: "Send the HTTP response for a waiting webhook trigger.",
	fallible: true,
	group: "actions",
	icon: Reply,
	kind: "action",
	label: "Webhook Response",
	permission: { name: "webhook_response", risk: "low" },
	risk: "low",
	runtimeOutputs: fallible([
		{
			name: "sent",
			type: "boolean",
			description: "Whether this node sent the webhook response.",
			example: "n-mr3zyt6f-22.sent",
		},
		{
			name: "status_code",
			type: "http_status_code",
			description: "HTTP response status code sent to the webhook caller.",
			example: "n-mr3zyt6f-22.status_code",
		},
		{
			name: "content_type",
			type: "string",
			description: "HTTP response content type.",
			example: "n-mr3zyt6f-22.content_type",
		},
		{
			name: "headers",
			type: "http_headers",
			description: "HTTP response headers sent to the webhook caller.",
			example: 'n-mr3zyt6f-22.headers["cache-control"]',
		},
		{ name: "body", type: "string", description: "HTTP response body.", example: "n-mr3zyt6f-22.body" },
		{
			name: "trigger_id",
			type: "string",
			description: "Webhook trigger node id that owns the pending response.",
			example: "n-mr3zyt6f-22.trigger_id",
		},
	]),
	runnerType: "webhook_response",
	validateConfig: (config) =>
		[
			requiredConfig(config, "statusCode", "webhook response status code"),
			validateHttpStatusConfig(config, "statusCode", "webhook response status code"),
			requiredConfig(config, "contentType", "webhook response content type"),
			validateHeaders(config.headers),
		].filter(Boolean),
	validateGraph: ({ context, node }) =>
		canBeReachedFromWaitingWebhook(node.id, context.edges, context.nodes)
			? []
			: [
					`${node.id} is a Webhook Response node, but it is not reachable from a Webhook Trigger with "Wait for response node" enabled.`,
				],
	simulation: {
		createOutput: ({ api, context, node }) => {
			if (!context.webhookResponse?.waiting) {
				return {
					failed: true,
					outputData: {
						error: api.createError(
							"Webhook Response reached without a waiting webhook request.",
							"WEBHOOK_RESPONSE_NOT_WAITING",
							"runtime",
							{ node_id: node.id },
							false,
						),
					},
				};
			}

			if (context.webhookResponse.sent) {
				return {
					failed: true,
					outputData: {
						error: api.createError(
							"Webhook response was already sent for this request.",
							"WEBHOOK_RESPONSE_ALREADY_SENT",
							"runtime",
							{ node_id: node.id, trigger_id: context.webhookResponse.triggerNodeId },
							false,
						),
					},
				};
			}

			const response = {
				body: String(api.resolveTemplate(api.getConfigString(node, "body"), context)),
				content_type:
					String(api.resolveTemplate(api.getConfigString(node, "contentType"), context)).trim() || "text/plain",
				headers: createResponseHeaders(node.data.config.headers, api, context),
				sent: true,
				status_code: normalizeHttpStatus(
					String(api.resolveTemplate(api.getConfigString(node, "statusCode"), context)),
					200,
				),
				trigger_id: context.webhookResponse.triggerNodeId,
			};

			context.webhookResponse.sent = true;
			context.webhookResponse.response = response;

			return {
				failed: false,
				outputData: response,
			};
		},
		describe: ({ context, failed, node }) => {
			const output = context.nodeOutputs[node.id] ?? {};
			if (failed) {
				const error =
					output.error && typeof output.error === "object" && !Array.isArray(output.error) ? output.error : {};
				return [
					{
						level: "error",
						message: `[Simulation] Webhook Response (${node.id}) failed. ${String(error.message ?? "No pending webhook response was available.")}`,
					},
				];
			}

			return [
				{
					level: "info",
					message: `[Simulation] Webhook Response (${node.id}) sent ${String(output.status_code ?? 200)} ${String(output.content_type ?? "text/plain")}: ${truncateResponseBody(String(output.body ?? ""))}`,
				},
			];
		},
	},
});

function validateHttpStatusConfig(config: Record<string, JsonValue>, key: string, label: string) {
	const value = configString(config, key).trim();
	if (!value || /\{\{\s*[^{}]+\s*}}/.test(value)) {
		return "";
	}

	const status = Number(value);
	return Number.isInteger(status) && status >= 100 && status <= 599 ? "" : `${label} must be an HTTP status 100-599.`;
}

function validateHeaders(value: JsonValue | undefined) {
	if (value === undefined) {
		return "";
	}

	if (!Array.isArray(value)) {
		return "webhook response headers must be a list of header rows.";
	}

	const invalidHeader = value.find((header) => !isHeaderRow(header) || !isValidHeaderName(header.name));
	return invalidHeader ? "webhook response header names must use valid HTTP header token syntax." : "";
}

function isValidHeaderName(value: string) {
	return !value.trim() || /^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/.test(value.trim());
}

function createResponseHeaders(value: JsonValue | undefined, api: NodeSimulationApi, context: SimulationContext) {
	const headers: Record<string, JsonValue> = {};
	if (!Array.isArray(value)) {
		return headers;
	}

	for (const header of value) {
		if (!isHeaderRow(header)) {
			continue;
		}

		const name = header.name.trim().toLowerCase();
		if (name) {
			headers[name] = String(api.resolveTemplate(header.value, context));
		}
	}

	return headers;
}

function normalizeHttpStatus(value: string, fallback: number) {
	const status = Number(value);
	return Number.isInteger(status) && status >= 100 && status <= 599 ? status : fallback;
}

function canBeReachedFromWaitingWebhook(
	targetNodeId: string,
	edges: { source: string; target: string }[],
	nodes: Array<{ id: string; data: { actionType: string; config: Record<string, JsonValue> } }>,
) {
	const waitingWebhooks = nodes.filter(
		(node) => node.data.actionType === "trigger.webhook" && isEnabled(node.data.config.waitForResponse),
	);

	return waitingWebhooks.some((webhook) =>
		canReachNode(webhook.id, targetNodeId, edges, new Set(nodes.map((node) => node.id))),
	);
}

function canReachNode(
	startNodeId: string,
	targetNodeId: string,
	edges: { source: string; target: string }[],
	nodeIds: ReadonlySet<string>,
) {
	const visited = new Set<string>();
	const queue = [startNodeId];

	while (queue.length > 0) {
		const nodeId = queue.shift();
		if (!nodeId || visited.has(nodeId) || !nodeIds.has(nodeId)) {
			continue;
		}

		if (nodeId === targetNodeId) {
			return true;
		}

		visited.add(nodeId);
		for (const edge of edges) {
			if (edge.source === nodeId) {
				queue.push(edge.target);
			}
		}
	}

	return false;
}

function isEnabled(value: JsonValue | undefined) {
	return value === true || value === "true";
}

function truncateResponseBody(value: string) {
	return value.length > 180 ? `${value.slice(0, 179)}...` : value;
}

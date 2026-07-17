import type { Node } from "@xyflow/react";
import { Globe } from "lucide-react";
import type { ScriptNodeData } from "@/lib/types";
import type { SimulationContext } from "@/utils/simulation-types";
import type { NodeSimulationApi } from "../../node-definition";
import { defineNode } from "../../node-definition";
import { httpMethodOptions } from "../options";
import { createHeaderRow } from "../rows";
import { fallible } from "../runtime-outputs";
import { requiredConfig, staticHttpUrlConfig, staticPositiveNumberConfig } from "../validators";

export const httpRequestNode = defineNode({
	actionType: "action.http",
	capabilities: ["action.http"],
	configFields: [
		{ key: "method", label: "Method", type: "select", options: httpMethodOptions },
		{ key: "url", label: "URL", type: "text", usesVariables: true },
		{ key: "userAgent", label: "User-Agent", type: "text", usesVariables: true },
		{
			key: "timeoutSeconds",
			label: "Timeout seconds",
			type: "number",
			numeric: {
				kind: "float",
				signed: false,
				minimum: "0",
				maximum: "86400",
				minimumInclusive: false,
				maximumInclusive: true,
			},
		},
		{ key: "body", label: "Body", type: "textarea", usesVariables: true },
	],
	defaultConfig: () => ({
		method: "GET",
		url: "https://example.com",
		headers: [createHeaderRow("Accept", "application/json"), createHeaderRow("Content-Type", "application/json")],
		userAgent: "BaudBound/{{manifest_name}}",
		timeoutSeconds: "30",
		body: "",
	}),
	description: "Send an HTTP request.",
	fallible: true,
	group: "actions",
	icon: Globe,
	kind: "action",
	label: "HTTP Request",
	permission: { name: "http_request", risk: "medium" },
	risk: "medium",
	runtimeOutputs: fallible([
		{
			name: "status_code",
			type: "http_status_code",
			description: "HTTP response status code.",
			example: "n-mr3zyt6f-12.status_code",
		},
		{
			name: "status_text",
			type: "string",
			description: "HTTP response status text.",
			example: "n-mr3zyt6f-12.status_text",
		},
		{
			name: "headers",
			type: "http_headers",
			description: "HTTP response headers.",
			example: 'n-mr3zyt6f-12.headers["content-type"]',
		},
		{ name: "body", type: "string", description: "Raw response body.", example: "n-mr3zyt6f-12.body" },
		{
			name: "json",
			type: "object",
			description: "Parsed JSON body when the response is JSON.",
			example: "n-mr3zyt6f-12.json.user.name",
		},
		{
			name: "duration_ms",
			type: "duration_ms",
			description: "Request duration in milliseconds.",
			example: "n-mr3zyt6f-12.duration_ms",
		},
	]),
	runnerType: "http_request",
	validateConfig: (config) =>
		[
			requiredConfig(config, "url", "request URL"),
			staticHttpUrlConfig(config, "url", "request URL"),
			staticPositiveNumberConfig(config, "timeoutSeconds", "timeout seconds"),
		].filter(Boolean),
	simulation: {
		createOutput: ({ api, context, node }) => api.executeHttpRequest(node, context),
		describe: ({ api, context, failed, node }) => [
			{
				level: failed ? "error" : "info",
				message: `[Simulation] HTTP Request (${node.id}) ${failed ? "failed" : "succeeded"}. ${getHttpExecutionDetail(api, node, context)}`,
			},
		],
	},
});

function getHttpExecutionDetail(api: NodeSimulationApi, node: Node<ScriptNodeData>, context: SimulationContext) {
	const output = context.nodeOutputs[node.id];
	const method = api.getConfigString(node, "method");
	const url = api.formatValue(api.resolveTemplate(api.getConfigString(node, "url"), context));

	if (output?.error && typeof output.error === "object" && !Array.isArray(output.error)) {
		return `${method} ${url} failed: ${String(output.error.message ?? "request failed")}.`;
	}

	if (typeof output?.status_code === "number") {
		return `${method} ${url} returned ${output.status_code} ${String(output.status_text ?? "")} in ${String(output.duration_ms ?? "?")}ms.`;
	}

	return `${method} ${url} was skipped because the simulation stopped.`;
}

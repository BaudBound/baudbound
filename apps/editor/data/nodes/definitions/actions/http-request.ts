import type { Node } from "@xyflow/react";
import { Globe } from "lucide-react";
import type { JsonValue, ScriptNodeData } from "@/lib/types";
import type { SimulationContext } from "@/utils/simulation-types";
import type { NodeSimulationApi } from "../../node-definition";
import { defineNode } from "../../node-definition";
import { httpBodyFormatOptions, httpMethodOptions } from "../options";
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
		{
			key: "bodyFormat",
			label: "Body format",
			type: "select",
			options: httpBodyFormatOptions,
			required: false,
			help: "JSON safely serializes variables. Text sends the resolved body without JSON processing.",
		},
		{ key: "body", label: "Body", type: "textarea", usesVariables: true },
	],
	defaultConfig: () => ({
		method: "GET",
		url: "https://example.com",
		headers: [createHeaderRow("Accept", "application/json"), createHeaderRow("Content-Type", "application/json")],
		userAgent: "BaudBound/{{manifest_name}}",
		timeoutSeconds: "30",
		bodyFormat: "json",
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
			validateHttpBody(config),
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

function validateHttpBody(config: Record<string, JsonValue>) {
	const body = typeof config.body === "string" ? config.body : "";
	if (!body.trim() || !usesJsonBody(config)) {
		return "";
	}

	try {
		JSON.parse(body);
		return "";
	} catch (error) {
		return `JSON request body is invalid: ${error instanceof Error ? error.message : String(error)}.`;
	}
}

function usesJsonBody(config: Record<string, JsonValue>) {
	if (config.bodyFormat === "json") {
		return true;
	}
	if (config.bodyFormat === "text") {
		return false;
	}

	return Array.isArray(config.headers) && config.headers.some(isJsonContentTypeHeader);
}

function isJsonContentTypeHeader(value: JsonValue) {
	if (!value || typeof value !== "object" || Array.isArray(value)) {
		return false;
	}
	const name = typeof value.name === "string" ? value.name : "";
	const headerValue = typeof value.value === "string" ? value.value : "";
	const mediaType = headerValue.split(";", 1)[0]?.trim().toLowerCase() ?? "";
	return (
		name.toLowerCase() === "content-type" &&
		(mediaType === "application/json" || (mediaType.startsWith("application/") && mediaType.endsWith("+json")))
	);
}

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

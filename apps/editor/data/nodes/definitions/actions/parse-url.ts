import { Link } from "lucide-react";
import type { JsonValue } from "@/lib/types";
import type { NodeExecutionResult } from "@/utils/simulation-types";
import { defineNode } from "../../node-definition";
import { fallible } from "../runtime-outputs";
import { requiredConfig } from "../validators";

export const parseUrlNode = defineNode({
	actionType: "action.url.parse",
	capabilities: ["action.text"],
	configFields: [{ key: "url", label: "URL", type: "textarea", usesVariables: true }],
	defaultConfig: () => ({ url: "https://example.com/path?param=value" }),
	description: "Parse a standard or custom absolute URL into its individual components.",
	fallible: true,
	group: "actions",
	icon: Link,
	kind: "action",
	label: "Parse URL",
	permission: { name: "parse_url", risk: "low" },
	risk: "low",
	runtimeOutputs: fallible([
		{
			name: "protocol",
			type: "string",
			description: "URL protocol without the trailing colon or slashes.",
			example: "n-mr3zyt6f-18.protocol",
		},
		{
			name: "host",
			type: "string",
			description: "URL host. Empty when the URL does not contain a host.",
			example: "n-mr3zyt6f-18.host",
		},
		{
			name: "port",
			type: "string",
			description: "Explicit URL port as text. Empty when no non-default port is present.",
			example: "n-mr3zyt6f-18.port",
		},
		{
			name: "path",
			type: "string",
			description: "URL path, including its leading slash when present.",
			example: "n-mr3zyt6f-18.path",
		},
		{
			name: "query",
			type: "string",
			description: "Raw URL query without the leading question mark.",
			example: "n-mr3zyt6f-18.query",
		},
		{
			name: "query_parameters",
			type: "list",
			description: "Ordered query parameters as objects containing name and value fields.",
			example: "n-mr3zyt6f-18.query_parameters",
		},
		{
			name: "fragment",
			type: "string",
			description: "URL fragment without the leading hash. Empty when no fragment is present.",
			example: "n-mr3zyt6f-18.fragment",
		},
	]),
	runnerType: "parse_url",
	validateConfig: (config) => {
		const requiredError = requiredConfig(config, "url", "URL");
		if (requiredError) {
			return [requiredError];
		}

		const value = configString(config.url).trim();
		if (containsVariable(value)) {
			return [];
		}

		const result = parseAbsoluteUrl(value);
		return result.ok ? [] : [result.error];
	},
	simulation: {
		createOutput: ({ api, context, node }): NodeExecutionResult => {
			const value = String(api.resolveTemplate(api.getConfigString(node, "url"), context));
			const result = parseAbsoluteUrl(value);
			if (!result.ok) {
				return {
					failed: true,
					outputData: {
						error: api.createError(result.error, "URL_PARSE_FAILED", "validation", { url: value }),
					},
				};
			}

			return { failed: false, outputData: result.output };
		},
		describe: ({ api, context, failed, node }) => {
			if (failed) {
				return [{ level: "error", message: `[Simulation] Parse URL (${node.id}) failed.` }];
			}

			const output = context.nodeOutputs[node.id];
			return [
				{
					level: "info",
					message: `[Simulation] Parsed URL as ${api.formatValue(output?.protocol ?? "")}://${api.formatValue(output?.host ?? "")}${api.formatValue(output?.path ?? "")}.`,
				},
			];
		},
	},
});

type ParseUrlResult = { ok: true; output: Record<string, JsonValue> } | { error: string; ok: false };

export function parseAbsoluteUrl(value: string): ParseUrlResult {
	try {
		const parsed = new URL(value);
		return {
			ok: true,
			output: {
				protocol: parsed.protocol.slice(0, -1),
				host: parsed.hostname,
				port: parsed.port,
				path: parsed.pathname,
				query: parsed.search.slice(1),
				query_parameters: Array.from(parsed.searchParams, ([name, parameterValue]) => ({
					name,
					value: parameterValue,
				})),
				fragment: parsed.hash.slice(1),
			},
		};
	} catch (error) {
		return {
			error: error instanceof Error ? `URL is invalid: ${error.message}` : "URL is invalid.",
			ok: false,
		};
	}
}

function containsVariable(value: string) {
	return /\{\{[^{}]+}}/.test(value);
}

function configString(value: JsonValue | undefined) {
	if (typeof value === "string") {
		return value;
	}

	if (value === undefined || value === null) {
		return "";
	}

	return String(value);
}

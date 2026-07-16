import { TextCursorInput } from "lucide-react";
import type { JsonValue } from "@/lib/types";
import { defineNode } from "../../node-definition";
import { textTransformOperationOptions } from "../options";
import { requiredConfig, staticNonNegativeIntegerConfig } from "../validators";

const textTransformOperations = textTransformOperationOptions.map((option) => option.value);
const unicodeLetterPattern = /\p{Alphabetic}/u;
const whitespacePattern = /\p{White_Space}/u;

type TextTransformOperation = (typeof textTransformOperations)[number];

export const formatTextNode = defineNode({
	actionType: "action.text.format",
	capabilities: ["action.text"],
	configFields: [
		{ key: "operation", label: "Operation", type: "select", options: textTransformOperationOptions, required: false },
		{ key: "template", label: "Template", type: "textarea", usesVariables: true, required: false },
		{ key: "input", label: "Input", type: "textarea", usesVariables: true, required: false },
		{ key: "search", label: "Search", type: "text", usesVariables: true, required: false },
		{ key: "replacement", label: "Replacement", type: "textarea", usesVariables: true, required: false },
		{ key: "delimiter", label: "Delimiter", type: "text", usesVariables: true, required: false },
		{ key: "items", label: "Items", type: "textarea", usesVariables: true, required: false },
		{ key: "start", label: "Start", type: "number", usesVariables: true, required: false },
		{ key: "length", label: "Length", type: "number", usesVariables: true, required: false },
		{ key: "targetLength", label: "Target length", type: "number", usesVariables: true, required: false },
		{ key: "pad", label: "Pad text", type: "text", usesVariables: true, required: false },
	],
	defaultConfig: () => ({
		operation: "template",
		template: "Hello {{item}}",
		input: "{{item}}",
		search: "",
		replacement: "",
		delimiter: ",",
		items: '["one","two","three"]',
		start: "0",
		length: "",
		targetLength: "10",
		pad: " ",
	}),
	description: "Transform text with templates, casing, replace, split, join, substring, padding, and encoding.",
	group: "actions",
	icon: TextCursorInput,
	kind: "action",
	label: "Text Transform",
	permission: { name: "text_transform", risk: "low" },
	risk: "low",
	runtimeOutputs: [
		{
			name: "text",
			type: "string",
			description: "Transformed text result. Empty when the selected operation only produces list output.",
			example: "n-mr3zyt6f-18.text",
		},
		{
			name: "items",
			type: "list",
			description: "List result for split operations, or the parsed list used by join.",
			example: "n-mr3zyt6f-18.items",
		},
	],
	runnerType: "format_text",
	sanitizeConfig: (config) => ({
		...config,
		operation: normalizeTextTransformOperation(configString(config.operation)),
	}),
	validateConfig: (config) => validateTextTransformConfig(config),
	simulation: {
		createOutput: ({ api, context, node }) => {
			const result = executeTextTransform({
				config: node.data.config,
				parseJsonValue: api.parseJsonValue,
				resolveTemplate: (value) => api.resolveTemplate(value, context),
			});

			if (!result.ok) {
				return {
					failed: true,
					outputData: {
						error: api.createError(result.error, "TEXT_TRANSFORM_FAILED", "validation", {
							operation: normalizeTextTransformOperation(configString(node.data.config.operation)),
						}),
					},
				};
			}

			return {
				failed: false,
				outputData: result.output,
			};
		},
		describe: ({ api, context, failed, node }) => {
			const operation = normalizeTextTransformOperation(api.getConfigString(node, "operation"));
			if (failed) {
				return [
					{
						level: "error",
						message: `[Simulation] Text Transform (${node.id}) failed while running ${formatOperationName(operation)}.`,
					},
				];
			}

			const output = context.nodeOutputs[node.id] ?? {};
			const text = typeof output.text === "string" ? output.text : "";
			const items = Array.isArray(output.items) ? output.items : [];
			const count = items.length;
			const suffix = operation === "split" || operation === "join" ? ` (${count} item${count === 1 ? "" : "s"})` : "";

			return [
				{
					level: "info",
					message: `[Simulation] Text Transform (${node.id}) ran ${formatOperationName(operation)}${suffix}. Result: "${truncateText(text, 160)}".`,
				},
			];
		},
	},
});

type ExecuteTextTransformParams = {
	config: Record<string, JsonValue>;
	parseJsonValue: (value: string) => JsonValue | undefined;
	resolveTemplate: (value: string) => JsonValue;
};

type TextTransformResult =
	| {
			ok: true;
			output: Record<string, JsonValue>;
	  }
	| {
			error: string;
			ok: false;
	  };

function validateTextTransformConfig(config: Record<string, JsonValue>) {
	const operation = normalizeTextTransformOperation(configString(config.operation));
	const errors = [validateOperation(configString(config.operation))];

	if (operation === "template") {
		errors.push(requiredConfig(config, "template", "text template"));
	}

	if (usesInput(operation)) {
		errors.push(requiredConfig(config, "input", "input text"));
	}

	if (operation === "replace" || operation === "regex_replace") {
		errors.push(requiredConfig(config, "search", operation === "replace" ? "search text" : "regex pattern"));
	}

	if (operation === "regex_replace") {
		errors.push(validateRegex(configString(config.search)));
	}

	if (operation === "split" || operation === "join") {
		errors.push(requiredConfig(config, "delimiter", "delimiter"));
	}

	if (operation === "join") {
		errors.push(validateJsonList(configString(config.items)));
	}

	if (operation === "substring") {
		errors.push(staticNonNegativeIntegerConfig(config, "start", "substring start"));
		if (configString(config.length).trim()) {
			errors.push(staticNonNegativeIntegerConfig(config, "length", "substring length"));
		}
	}

	if (operation === "pad_start" || operation === "pad_end") {
		errors.push(staticNonNegativeIntegerConfig(config, "targetLength", "target length"));
		errors.push(requiredConfig(config, "pad", "pad text"));
	}

	return errors.filter(Boolean);
}

export function executeTextTransform({
	config,
	parseJsonValue,
	resolveTemplate,
}: ExecuteTextTransformParams): TextTransformResult {
	const configuredOperation = configString(config.operation);
	if (configuredOperation && !textTransformOperations.includes(configuredOperation)) {
		return { error: `Unsupported text transform operation "${configuredOperation}".`, ok: false };
	}
	const operation = normalizeTextTransformOperation(configuredOperation);
	const input = resolveToString(configString(config.input), resolveTemplate);
	const template = resolveToString(configString(config.template), resolveTemplate);
	const search = resolveToString(configString(config.search), resolveTemplate);
	const replacement = resolveToString(configString(config.replacement), resolveTemplate);
	const delimiter = resolveToString(configString(config.delimiter), resolveTemplate);
	const pad = resolveToString(configString(config.pad), resolveTemplate) || " ";

	try {
		if (operation === "template") {
			return createTextOutput(template);
		}

		if (operation === "trim") {
			return createTextOutput(input.trim());
		}

		if (operation === "uppercase") {
			return createTextOutput(input.toUpperCase());
		}

		if (operation === "lowercase") {
			return createTextOutput(input.toLowerCase());
		}

		if (operation === "sentence_case") {
			return createTextOutput(toSentenceCase(input));
		}

		if (operation === "capitalize_words") {
			return createTextOutput(capitalizeWords(input));
		}

		if (operation === "replace") {
			return createTextOutput(input.replaceAll(search, replacement));
		}

		if (operation === "regex_replace") {
			return createTextOutput(input.replace(new RegExp(search, "gu"), replacement));
		}

		if (operation === "split") {
			const items = input.split(delimiter);
			return createTextOutput("", items);
		}

		if (operation === "join") {
			const items = parseItems(configString(config.items), parseJsonValue, resolveTemplate);
			if (!items.ok) {
				return items;
			}

			return createTextOutput(items.items.map((item) => stringifyItem(item)).join(delimiter), items.items);
		}

		if (operation === "substring") {
			const start = normalizeInteger(resolveToString(configString(config.start), resolveTemplate), 0);
			const rawLength = resolveToString(configString(config.length), resolveTemplate).trim();
			const length = rawLength ? Math.max(0, normalizeInteger(rawLength, 0)) : undefined;
			return createTextOutput(substringByCodePoints(input, start, length));
		}

		if (operation === "pad_start") {
			const targetLength = normalizeInteger(
				resolveToString(configString(config.targetLength), resolveTemplate),
				codePointLength(input),
			);
			return createTextOutput(padByCodePoints(input, targetLength, pad, true));
		}

		if (operation === "pad_end") {
			const targetLength = normalizeInteger(
				resolveToString(configString(config.targetLength), resolveTemplate),
				codePointLength(input),
			);
			return createTextOutput(padByCodePoints(input, targetLength, pad, false));
		}

		if (operation === "url_encode") {
			return createTextOutput(encodeURIComponent(input));
		}

		if (operation === "url_decode") {
			return createTextOutput(decodeURIComponent(input));
		}

		if (operation === "base64_encode") {
			return createTextOutput(encodeBase64(input));
		}

		if (operation === "base64_decode") {
			return createTextOutput(decodeBase64(input));
		}

		if (operation === "json_escape") {
			return createTextOutput(JSON.stringify(input));
		}

		if (operation === "json_unescape") {
			const parsed = JSON.parse(input);
			return createTextOutput(typeof parsed === "string" ? parsed : stringifyItem(parsed));
		}

		return { error: `Unsupported text transform operation "${operation}".`, ok: false };
	} catch (error) {
		return {
			error: error instanceof Error ? error.message : "Text transform failed.",
			ok: false,
		};
	}
}

function createTextOutput(text: string, items: JsonValue[] = []) {
	return {
		ok: true,
		output: {
			text,
			items,
		},
	} satisfies TextTransformResult;
}

function parseItems(
	rawItems: string,
	parseJsonValue: (value: string) => JsonValue | undefined,
	resolveTemplate: (value: string) => JsonValue,
): { items: JsonValue[]; ok: true } | { error: string; ok: false } {
	const resolved = resolveTemplate(rawItems);
	if (Array.isArray(resolved)) {
		return { items: resolved, ok: true };
	}

	const parsed = typeof resolved === "string" ? parseJsonValue(resolved) : resolved;
	if (Array.isArray(parsed)) {
		return { items: parsed, ok: true };
	}

	return { error: "Join items must be a JSON array or a reference that resolves to a list.", ok: false };
}

function usesInput(operation: TextTransformOperation) {
	return operation !== "template" && operation !== "join";
}

function validateOperation(operation: string) {
	return textTransformOperations.includes(operation) || operation === ""
		? ""
		: `Unsupported text operation "${operation}".`;
}

function validateRegex(pattern: string) {
	if (!pattern.trim()) {
		return "";
	}

	try {
		new RegExp(pattern, "u");
		return "";
	} catch (error) {
		return error instanceof Error ? `Regex pattern is invalid: ${error.message}` : "Regex pattern is invalid.";
	}
}

function validateJsonList(value: string) {
	const trimmed = value.trim();
	if (!trimmed || /^\{\{[^{}]+}}$/.test(trimmed)) {
		return "";
	}

	try {
		return Array.isArray(JSON.parse(trimmed)) ? "" : "Join items must be a JSON array.";
	} catch {
		return "Join items must be valid JSON or a list reference.";
	}
}

function normalizeTextTransformOperation(value: string): TextTransformOperation {
	return textTransformOperations.includes(value) ? value : "template";
}

function resolveToString(value: string, resolveTemplate: (value: string) => JsonValue) {
	const resolved = resolveTemplate(value);
	return typeof resolved === "string" ? resolved : stringifyItem(resolved);
}

function stringifyItem(value: JsonValue) {
	if (typeof value === "string") {
		return value;
	}

	if (value === undefined) {
		return "";
	}

	return JSON.stringify(value);
}

function normalizeInteger(value: string, fallback: number) {
	const parsed = Number(value);
	return Number.isFinite(parsed) ? Math.trunc(parsed) : fallback;
}

function codePointLength(value: string) {
	return Array.from(value).length;
}

function toSentenceCase(value: string) {
	const [first, ...rest] = Array.from(value);
	return first === undefined ? "" : `${first.toUpperCase()}${rest.join("").toLowerCase()}`;
}

function capitalizeWords(value: string) {
	let waitingForFirstLetter = true;
	let result = "";

	for (const character of value) {
		if (whitespacePattern.test(character)) {
			waitingForFirstLetter = true;
			result += character;
			continue;
		}

		if (!unicodeLetterPattern.test(character)) {
			result += character;
			continue;
		}

		result += waitingForFirstLetter ? character.toUpperCase() : character.toLowerCase();
		waitingForFirstLetter = false;
	}

	return result;
}

function substringByCodePoints(value: string, start: number, length?: number) {
	const codePoints = Array.from(value);
	return codePoints.slice(start, length === undefined ? undefined : start + length).join("");
}

function padByCodePoints(value: string, targetLength: number, pad: string, atStart: boolean) {
	const missing = Math.max(0, targetLength - codePointLength(value));
	const padCodePoints = Array.from(pad);
	if (missing === 0 || padCodePoints.length === 0) {
		return value;
	}

	const repeated = Array.from({ length: missing }, (_, index) => padCodePoints[index % padCodePoints.length]).join("");
	return atStart ? `${repeated}${value}` : `${value}${repeated}`;
}

function encodeBase64(value: string) {
	const bytes = new TextEncoder().encode(value);
	let binary = "";
	for (const byte of bytes) {
		binary += String.fromCharCode(byte);
	}

	return btoa(binary);
}

function decodeBase64(value: string) {
	const encoded = value.trim();
	if (!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(encoded)) {
		throw new Error("Invalid Base64 input. Use standard padded Base64 without embedded whitespace.");
	}

	const binary = atob(encoded);
	if (btoa(binary) !== encoded) {
		throw new Error("Invalid Base64 input. The final Base64 character contains non-zero trailing bits.");
	}
	const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
	return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
}

function formatOperationName(operation: string) {
	return operation.replaceAll("_", " ");
}

function truncateText(value: string, maxLength: number) {
	return value.length > maxLength ? `${value.slice(0, maxLength - 1)}...` : value;
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

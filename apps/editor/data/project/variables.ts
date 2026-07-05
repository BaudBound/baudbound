import type { Node } from "@xyflow/react";
import type { JsonValue, RuntimeDataType, ScriptNodeData } from "@/lib/types";

export const variableTypes = [
	"string",
	"number",
	"boolean",
	"object",
	"list",
	"http_response",
	"datetime",
	"duration",
	"file_path",
] as const;

export type VariableType = (typeof variableTypes)[number];

export const variableScopes = ["runtime", "persistent", "global", "secret"] as const;

export type VariableScope = (typeof variableScopes)[number];

export const variableOperations = ["set", "increment", "append_list", "set_object_field", "clear"] as const;

export type VariableOperation = (typeof variableOperations)[number];

export const reservedVariablePrefixes = ["manifest_", "system_"] as const;

export type EditorVariableScope = VariableScope | "manifest" | "system" | "node_output";

export type EditorVariableSource = "user" | "built_in" | "node_output";

export type EditorVariable<TValue extends JsonValue | undefined = JsonValue | undefined> = {
	description?: string;
	name: string;
	read_only: boolean;
	scope: EditorVariableScope;
	source: EditorVariableSource;
	token: string;
	type: VariableType | RuntimeDataType;
	value?: TValue;
};

export const variableTypeDefinitions: Record<VariableType, { description: string; example: string }> = {
	string: {
		description: "Plain text. Variables are allowed inside the text.",
		example: "server ok",
	},
	number: {
		description: "A finite numeric value.",
		example: "42",
	},
	boolean: {
		description: "A true or false value.",
		example: "true",
	},
	object: {
		description: "A JSON object.",
		example: formatJson({ status: "ok", count: 1 }),
	},
	list: {
		description: "A JSON array.",
		example: formatJson(["api", "db", "cache"]),
	},
	http_response: {
		description: "A JSON HTTP response object with status, headers, and body.",
		example: formatJson({
			type: "http_response",
			status: 200,
			headers: { "content-type": "application/json" },
			body: "{}",
		}),
	},
	datetime: {
		description: "A JSON datetime object with an ISO-8601 value.",
		example: formatJson({ type: "datetime", value: "2026-07-02T12:00:00Z" }),
	},
	duration: {
		description: "A JSON duration object with a unit and numeric value.",
		example: formatJson({ type: "duration", unit: "seconds", value: 10 }),
	},
	file_path: {
		description: "A file path string. Relative paths are resolved by the runner.",
		example: "./logs/output.txt",
	},
};

export const variableScopeDefinitions: Record<VariableScope, string> = {
	runtime: "Exists only during one script run.",
	persistent: "Stored between runs.",
	global: "Configured globally and available to scripts.",
	secret: "Sensitive encrypted value such as an API key.",
};

export const variableOperationDefinitions: Record<
	VariableOperation,
	{ description: string; label: string; valueLabel: string }
> = {
	set: {
		label: "Set",
		valueLabel: "Value",
		description: "Create or replace the variable value.",
	},
	increment: {
		label: "Increment",
		valueLabel: "Amount",
		description: "Add a numeric amount to an existing or newly created number variable.",
	},
	append_list: {
		label: "Append list",
		valueLabel: "Item",
		description: "Append one item to a list variable. The target variable type must be list.",
	},
	set_object_field: {
		label: "Set object field",
		valueLabel: "Field value",
		description: "Set a nested field inside an object variable. Missing object fields may be created by the runner.",
	},
	clear: {
		label: "Clear",
		valueLabel: "Clear value",
		description: "Reset the variable to the empty value for its type.",
	},
};

export function getDefaultVariableValue(type: VariableType) {
	return variableTypeDefinitions[type].example;
}

export function getDefaultVariableOperationValue(operation: VariableOperation, type: VariableType) {
	if (operation === "increment") {
		return "1";
	}

	if (operation === "append_list") {
		return "item";
	}

	if (operation === "set_object_field") {
		return "value";
	}

	if (operation === "clear") {
		return getClearedVariableValue(type);
	}

	return getDefaultVariableValue(type);
}

export function getVariableOperationFixedType(operation: VariableOperation): VariableType | null {
	if (operation === "increment") {
		return "number";
	}

	if (operation === "append_list") {
		return "list";
	}

	if (operation === "set_object_field") {
		return "object";
	}

	return null;
}

export function getClearedVariableValue(type: VariableType) {
	if (type === "number") {
		return "0";
	}

	if (type === "boolean") {
		return "false";
	}

	if (type === "list") {
		return "[]";
	}

	if (type === "object") {
		return "{}";
	}

	if (type === "duration") {
		return formatJson({ type: "duration", unit: "seconds", value: 0 });
	}

	if (type === "datetime") {
		return formatJson({ type: "datetime", value: "1970-01-01T00:00:00.000Z" });
	}

	if (type === "http_response") {
		return formatJson({ type: "http_response", status: 0, headers: {}, body: "" });
	}

	return "";
}

export function validateVariableOperationValue(
	operation: VariableOperation,
	type: VariableType,
	value: string,
	path = "",
) {
	if (operation === "clear") {
		return "";
	}

	const compatibilityMessage = validateVariableOperationType(operation, type);
	if (compatibilityMessage) {
		return compatibilityMessage;
	}

	if (operation === "increment") {
		return validateVariableValue("number", value).replace("Number variables", "Increment amount");
	}

	if (operation === "append_list") {
		return validateJsonCompatibleValue(value);
	}

	if (operation === "set_object_field") {
		const pathMessage = validateObjectFieldPath(path);
		return pathMessage || validateJsonCompatibleValue(value);
	}

	return validateVariableValue(type, value);
}

export function validateVariableOperationType(operation: VariableOperation, type: VariableType) {
	if (operation === "increment" && type !== "number") {
		return "Increment can only be used with number variables.";
	}

	if (operation === "append_list" && type !== "list") {
		return "Append list can only be used with list variables.";
	}

	const fixedType = getVariableOperationFixedType(operation);
	if (fixedType && type !== fixedType) {
		return `${variableOperationDefinitions[operation].label} can only be used with ${fixedType} variables.`;
	}

	return "";
}

export function validateObjectFieldPath(path: string) {
	const trimmed = path.trim();

	if (!trimmed) {
		return "Object field path is required.";
	}

	return /^[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*|\[(?:0|[1-9][0-9]*)\])*$/.test(trimmed)
		? ""
		: "Object field path must use dot fields and numeric indexes, for example profile.name or users[0].name.";
}

export function normalizeVariableOperation(value: string): VariableOperation {
	return variableOperations.includes(value as VariableOperation) ? (value as VariableOperation) : "set";
}

export function validateVariableValue(type: VariableType, value: string) {
	const trimmed = value.trim();

	if (isTemplateReference(trimmed)) {
		return "";
	}

	if (type === "string") {
		return "";
	}

	if (type === "number") {
		return Number.isFinite(Number(trimmed)) ? "" : "Number variables must be a finite number, for example 42.";
	}

	if (type === "boolean") {
		return trimmed === "true" || trimmed === "false" ? "" : "Boolean variables must be true or false.";
	}

	if (type === "file_path") {
		return trimmed.length > 0 ? "" : "File path variables must not be empty.";
	}

	const parsed = parseJson(trimmed);
	if (!parsed.ok) {
		return "This variable type must be valid JSON.";
	}

	if (type === "list") {
		return Array.isArray(parsed.value) ? "" : "List variables must be a JSON array.";
	}

	if (type === "object") {
		return isRecord(parsed.value) ? "" : "Object variables must be a JSON object.";
	}

	if (!isRecord(parsed.value)) {
		return "This variable type must be a JSON object.";
	}

	if (type === "duration") {
		return parsed.value.type === "duration" &&
			typeof parsed.value.unit === "string" &&
			typeof parsed.value.value === "number"
			? ""
			: "Duration variables must include type, unit, and numeric value fields.";
	}

	if (type === "datetime") {
		const valueField = parsed.value.value;
		return parsed.value.type === "datetime" && typeof valueField === "string" && !Number.isNaN(Date.parse(valueField))
			? ""
			: "Datetime variables must include type and a valid ISO-8601 value field.";
	}

	if (type === "http_response") {
		return parsed.value.type === "http_response" &&
			typeof parsed.value.status === "number" &&
			isRecord(parsed.value.headers) &&
			"body" in parsed.value
			? ""
			: "HTTP response variables must include type, status, headers, and body fields.";
	}

	return "";
}

export function validateVariableName(name: string) {
	const trimmed = name.trim();

	if (!trimmed) {
		return "Variable name is required.";
	}

	if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(trimmed)) {
		return "Variable names must start with a letter or underscore and only use letters, numbers, or underscores.";
	}

	const reservedPrefix = reservedVariablePrefixes.find((prefix) => trimmed.startsWith(prefix));
	return reservedPrefix ? `Variable names starting with "${reservedPrefix}" are reserved.` : "";
}

export function validateWritableVariableName(name: string, readOnlyNames: ReadonlySet<string>) {
	const validationMessage = validateVariableName(name);
	if (validationMessage) {
		return validationMessage;
	}

	const normalizedName = normalizeVariableReferenceName(name);
	return readOnlyNames.has(normalizedName) ? `Variable "${normalizedName}" is read-only and cannot be changed.` : "";
}

export function normalizeVariableReferenceName(name: string) {
	const trimmed = name.trim();
	const templateMatch = trimmed.match(/^\{\{\s*([^{}]+?)\s*\}\}$/);

	return templateMatch ? templateMatch[1].trim() : trimmed;
}

export function createNodeOutputVariables(nodes: Node<ScriptNodeData>[]): EditorVariable[] {
	return nodes.flatMap((node) =>
		(node.data.runtimeOutputs ?? []).flatMap((output) => {
			const outputName = `${node.id}.${output.name}`;
			const outputVariable: EditorVariable = {
				description: output.description,
				name: outputName,
				read_only: true,
				scope: "node_output",
				source: "node_output",
				token: `{{${outputName}}}`,
				type: output.type,
				value: undefined,
			};
			const fieldVariables =
				output.fields?.map((field) => {
					const fieldName = `${outputName}.${field.name}`;
					return {
						description: field.description,
						name: fieldName,
						read_only: true,
						scope: "node_output",
						source: "node_output",
						token: `{{${fieldName}}}`,
						type: field.type,
						value: undefined,
					} satisfies EditorVariable;
				}) ?? [];

			return [outputVariable, ...fieldVariables];
		}),
	);
}

export function createConfiguredVariableDefinitions(nodes: Node<ScriptNodeData>[]): EditorVariable[] {
	const variables = new Map<string, EditorVariable>();

	for (const node of nodes) {
		if (node.data.actionType !== "runtime.set_variable") {
			continue;
		}

		const name = normalizeVariableReferenceName(configString(node.data.config.name));
		if (!name || validateVariableName(name)) {
			continue;
		}

		const operation = normalizeVariableOperation(configString(node.data.config.operation));

		variables.set(name, {
			description: `Written by Variable Operation node ${node.id} using ${variableOperationDefinitions[operation].label}.`,
			name,
			read_only: false,
			scope: normalizeVariableScope(configString(node.data.config.scope)),
			source: "user",
			token: `{{${name}}}`,
			type: normalizeVariableType(configString(node.data.config.valueType)),
			value: undefined,
		});
	}

	return [...variables.values()].sort((a, b) => a.name.localeCompare(b.name));
}

export function createDerivedVariableMetadataDefinitions(variables: EditorVariable[]): EditorVariable[] {
	return variables.flatMap((variable) => {
		if (variable.name.includes(".$")) {
			return [];
		}

		return derivedVariableMetadataFields.map((field) => {
			const name = `${variable.name}.${field.name}`;
			return {
				description: `${field.description} Derived from ${variable.name}.`,
				name,
				read_only: true,
				scope: variable.scope,
				source: variable.source,
				token: `{{${name}}}`,
				type: field.type,
				value: getDerivedVariableMetadataValue(variable.value, field.name),
			} satisfies EditorVariable;
		});
	});
}

const derivedVariableMetadataFields = [
	{
		name: "$length",
		type: "number",
		description: "Length for strings, item count for lists, key count for objects.",
	},
	{
		name: "$count",
		type: "number",
		description: "Alias for $length.",
	},
	{
		name: "$type",
		type: "string",
		description: "Current value type.",
	},
	{
		name: "$is_empty",
		type: "boolean",
		description: "Whether the value is empty, null, or missing.",
	},
] as const;

function getDerivedVariableMetadataValue(
	value: JsonValue | undefined,
	field: (typeof derivedVariableMetadataFields)[number]["name"],
) {
	if (value === undefined) {
		return undefined;
	}

	if (field === "$length" || field === "$count") {
		return getValueLength(value);
	}

	if (field === "$type") {
		return getValueType(value);
	}

	return isValueEmpty(value);
}

function getValueLength(value: JsonValue | undefined) {
	if (typeof value === "string" || Array.isArray(value)) {
		return value.length;
	}

	if (value && typeof value === "object") {
		return Object.keys(value).length;
	}

	return 0;
}

function getValueType(value: JsonValue | undefined) {
	if (Array.isArray(value)) {
		return "list";
	}

	if (value === null) {
		return "null";
	}

	if (value === undefined) {
		return "missing";
	}

	return typeof value;
}

function isValueEmpty(value: JsonValue | undefined) {
	if (value === undefined || value === null) {
		return true;
	}

	if (typeof value === "string" || Array.isArray(value)) {
		return value.length === 0;
	}

	if (typeof value === "object") {
		return Object.keys(value).length === 0;
	}

	return false;
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

function normalizeVariableScope(value: string): VariableScope {
	return variableScopes.includes(value as VariableScope) ? (value as VariableScope) : "runtime";
}

function normalizeVariableType(value: string): VariableType {
	return variableTypes.includes(value as VariableType) ? (value as VariableType) : "string";
}

function validateJsonCompatibleValue(value: string) {
	const trimmed = value.trim();

	if (isTemplateReference(trimmed)) {
		return "";
	}

	if (!trimmed) {
		return "";
	}

	if (trimmed === "true" || trimmed === "false" || trimmed === "null" || Number.isFinite(Number(trimmed))) {
		return "";
	}

	if (trimmed.startsWith("{") || trimmed.startsWith("[")) {
		return parseJson(trimmed).ok ? "" : "Value must be valid JSON.";
	}

	return "";
}

function parseJson(value: string): { ok: true; value: unknown } | { ok: false } {
	try {
		return { ok: true, value: JSON.parse(value) };
	} catch {
		return { ok: false };
	}
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isTemplateReference(value: string) {
	return /^\{\{\s*[^{}]+\s*\}\}$/.test(value);
}

function formatJson(value: unknown) {
	return JSON.stringify(value, null, 2);
}

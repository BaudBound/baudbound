import {
	getClearedVariableValue,
	getDefaultVariableValue,
	type VariableType,
	validateVariableName,
	validateVariableValue,
} from "@/data/project/variables";
import type { DefaultVariable, JsonValue, SecretDeclaration } from "@/lib/types";

export function createDefaultValue(type: VariableType): JsonValue {
	const raw = type === "file_path" ? getDefaultVariableValue(type) : getClearedVariableValue(type);
	return parseDefaultValue(type, raw) ?? "";
}

export function formatDefaultValue(type: VariableType, value: JsonValue) {
	if (type === "string" || type === "file_path") {
		return typeof value === "string" ? value : "";
	}
	if (type === "number" || type === "boolean") {
		return String(value);
	}
	return JSON.stringify(value, null, 2);
}

export function parseDefaultValue(type: VariableType, rawValue: string): JsonValue | undefined {
	if (type === "string" || type === "file_path") {
		return rawValue;
	}
	if (type === "number") {
		const value = Number(rawValue.trim());
		return rawValue.trim() && Number.isFinite(value) ? value : undefined;
	}
	if (type === "boolean") {
		return rawValue === "true" ? true : rawValue === "false" ? false : undefined;
	}
	try {
		return JSON.parse(rawValue) as JsonValue;
	} catch {
		return undefined;
	}
}

export function defaultValueError(type: VariableType, rawValue: string) {
	if (!rawValue.trim()) {
		return "Default value is required.";
	}
	const parsed = parseDefaultValue(type, rawValue);
	if (parsed === undefined) {
		return type === "number" ? "Enter a finite number." : "Enter valid JSON.";
	}
	const validation = validateVariableValue(type, formatDefaultValue(type, parsed));
	return validation || null;
}

export function validateDefaultVariable(
	variable: DefaultVariable,
	existing: DefaultVariable[],
	secrets: SecretDeclaration[],
	originalName?: string,
) {
	const name = variable.name.trim();
	const nameError = validateVariableName(name);
	if (nameError) return nameError;
	if (existing.some((candidate) => candidate.name === name && candidate.name !== originalName)) {
		return `A default variable named "${name}" already exists.`;
	}
	if (secrets.some((secret) => secret.name === name)) {
		return `A secret named "${name}" already exists.`;
	}
	return null;
}

import { type VariableType, validateVariableName } from "@/data/project/variables";
import type { JsonValue, SecretDeclaration } from "@/lib/types";

export function validateSecretDeclaration(
	declaration: SecretDeclaration,
	existing: SecretDeclaration[],
	originalName?: string,
	reservedVariableNames: ReadonlySet<string> = new Set(),
) {
	const nameError = validateVariableName(declaration.name);
	if (nameError) {
		return nameError;
	}
	if (existing.some((secret) => secret.name === declaration.name && secret.name !== originalName)) {
		return `A secret named "${declaration.name}" already exists.`;
	}
	if (reservedVariableNames.has(declaration.name)) {
		return `A default variable named "${declaration.name}" already exists.`;
	}
	return null;
}

export function parseSecretSimulationValue(type: VariableType, rawValue: string): JsonValue | undefined {
	if (rawValue === "") {
		return undefined;
	}
	if (type === "string" || type === "file_path") {
		return rawValue;
	}
	if (type === "number") {
		const value = Number(rawValue);
		return Number.isFinite(value) ? value : undefined;
	}
	if (type === "boolean") {
		return rawValue === "true" ? true : rawValue === "false" ? false : undefined;
	}

	try {
		const value = JSON.parse(rawValue) as JsonValue;
		if (type === "list") {
			return Array.isArray(value) ? value : undefined;
		}
		return value !== null && typeof value === "object" && !Array.isArray(value) ? value : undefined;
	} catch {
		return undefined;
	}
}

export function secretSimulationValueError(type: VariableType, rawValue: string) {
	if (rawValue === "" || parseSecretSimulationValue(type, rawValue) !== undefined) {
		return null;
	}
	if (type === "number") return "Enter a finite number.";
	if (type === "boolean") return 'Enter "true" or "false".';
	if (type === "list") return "Enter a valid JSON array.";
	return "Enter a valid JSON object.";
}

export function createSimulationSecretValues(declarations: SecretDeclaration[], rawValues: Record<string, string>) {
	return Object.fromEntries(
		declarations.flatMap((declaration) => {
			const value = parseSecretSimulationValue(declaration.type, rawValues[declaration.name] ?? "");
			return value === undefined ? [] : [[declaration.name, value]];
		}),
	);
}

export function getSecretSimulationProblems(declarations: SecretDeclaration[], rawValues: Record<string, string>) {
	return declarations.flatMap((declaration) => {
		const rawValue = rawValues[declaration.name] ?? "";
		if (declaration.required && rawValue === "") {
			return [`Required simulation secret "${declaration.name}" has no value.`];
		}
		const error = secretSimulationValueError(declaration.type, rawValue);
		return error ? [`Simulation secret "${declaration.name}": ${error}`] : [];
	});
}

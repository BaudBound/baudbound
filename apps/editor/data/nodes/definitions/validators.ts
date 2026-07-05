import type { JsonValue } from "@/lib/types";

export function configString(config: Record<string, JsonValue>, key: string) {
	const value = config[key];
	if (typeof value === "string") {
		return value;
	}

	if (value === undefined || value === null) {
		return "";
	}

	return String(value);
}

export function requiredConfig(config: Record<string, JsonValue>, key: string, label: string) {
	return configString(config, key).trim() ? "" : `must define ${label}.`;
}

export function staticNonNegativeNumberConfig(config: Record<string, JsonValue>, key: string, label: string) {
	const value = configString(config, key).trim();
	if (!value) {
		return `must define ${label}.`;
	}

	if (hasTemplateReference(value)) {
		return "";
	}

	const numberValue = Number(value);
	return Number.isFinite(numberValue) && numberValue >= 0 ? "" : `${label} must be a non-negative number.`;
}

export function staticNonNegativeIntegerConfig(config: Record<string, JsonValue>, key: string, label: string) {
	const value = configString(config, key).trim();
	if (!value) {
		return `must define ${label}.`;
	}

	if (hasTemplateReference(value)) {
		return "";
	}

	const numberValue = Number(value);
	return Number.isInteger(numberValue) && numberValue >= 0 ? "" : `${label} must be a non-negative integer.`;
}

export function staticPositiveNumberConfig(config: Record<string, JsonValue>, key: string, label: string) {
	const value = configString(config, key).trim();
	if (!value) {
		return `must define ${label}.`;
	}

	if (hasTemplateReference(value)) {
		return "";
	}

	const numberValue = Number(value);
	return Number.isFinite(numberValue) && numberValue > 0 ? "" : `${label} must be greater than zero.`;
}

export function staticNumberConfig(config: Record<string, JsonValue>, key: string, label: string) {
	const value = configString(config, key).trim();
	if (!value) {
		return `must define ${label}.`;
	}

	if (hasTemplateReference(value)) {
		return "";
	}

	const numberValue = Number(value);
	return Number.isFinite(numberValue) ? "" : `${label} must be a number.`;
}

export function staticHttpUrlConfig(config: Record<string, JsonValue>, key: string, label: string) {
	const value = configString(config, key).trim();
	if (!value || hasTemplateReference(value)) {
		return "";
	}

	try {
		const url = new URL(value);
		return url.protocol === "http:" || url.protocol === "https:" ? "" : `${label} must use http:// or https://.`;
	} catch {
		return `${label} must be a valid URL.`;
	}
}

export function hasTemplateReference(value: string) {
	return /\{\{\s*[^{}]+\s*\}\}/.test(value);
}

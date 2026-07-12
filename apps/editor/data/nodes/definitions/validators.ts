import type { JsonValue, TargetRuntime } from "@/lib/types";

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

export function requiredStaticConfig(config: Record<string, JsonValue>, key: string, label: string) {
	const value = configString(config, key).trim();
	if (!value) {
		return `must define ${label}.`;
	}

	return hasTemplateReference(value) ? `${label} cannot use runtime variable references.` : "";
}

export function windowsDesktopOnlyConfigValue(
	config: Record<string, JsonValue>,
	key: string,
	value: string,
	targetRuntime: TargetRuntime,
	label: string,
) {
	return configString(config, key) === value && targetRuntime !== "Windows Desktop"
		? `${label} requires the Windows Desktop target runtime.`
		: "";
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

export function staticOptionalNumberRangeConfig(
	config: Record<string, JsonValue>,
	key: string,
	label: string,
	minimum: number,
	maximum: number,
) {
	const value = configString(config, key).trim();
	if (!value || hasTemplateReference(value)) {
		return "";
	}

	const numberValue = Number(value);
	return Number.isFinite(numberValue) && numberValue >= minimum && numberValue <= maximum
		? ""
		: `${label} must be between ${minimum} and ${maximum}.`;
}

export function staticPositiveDurationConfig(
	config: Record<string, JsonValue>,
	valueKey: string,
	unitKey: string,
	label: string,
) {
	const value = configString(config, valueKey).trim();
	if (!value) {
		return `must define ${label}.`;
	}
	if (hasTemplateReference(value)) {
		return `${label} cannot use runtime variable references.`;
	}

	const multiplier = {
		seconds: 1,
		minutes: 60,
		hours: 60 * 60,
		days: 24 * 60 * 60,
	}[configString(config, unitKey).trim()];
	const numericValue = Number(value);
	const seconds = numericValue * (multiplier ?? Number.NaN);
	if (!Number.isFinite(numericValue) || numericValue <= 0) {
		return `${label} must be greater than zero.`;
	}
	if (!Number.isFinite(seconds) || seconds < 1e-9 || seconds >= 2 ** 64) {
		return `${label} must fit the supported duration range and be at least one nanosecond.`;
	}
	return "";
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

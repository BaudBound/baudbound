import type { JsonValue } from "@/lib/types";

export type ColorComparisonMode = "per_channel" | "total_distance";

export type ColorMatchEvaluation = {
	blue_difference: number;
	difference_percent: number;
	green_difference: number;
	matches: boolean;
	red_difference: number;
};

type RgbColor = {
	b: number;
	g: number;
	r: number;
};

const HEX_COLOR = /^#([0-9a-f]{6})$/i;
const RGB_COLOR = /^rgb\(\s*([0-9]+)\s*,\s*([0-9]+)\s*,\s*([0-9]+)\s*\)$/i;
const TEMPLATE_REFERENCE = /\{\{\s*[^{}]+\s*\}\}/;
const MAX_TOTAL_DISTANCE = Math.sqrt(3 * 255 ** 2);

export function evaluateColorMatch(
	actualValue: JsonValue,
	expectedValue: JsonValue,
	mode: string,
	tolerancePercent: number,
): { error: string; ok: false } | { ok: true; value: ColorMatchEvaluation } {
	if (mode !== "per_channel" && mode !== "total_distance") {
		return { error: `comparison mode ${JSON.stringify(mode)} is unsupported`, ok: false };
	}
	if (!Number.isFinite(tolerancePercent) || tolerancePercent < 0 || tolerancePercent > 100) {
		return { error: "tolerance must be a finite percentage from 0 through 100", ok: false };
	}

	const actual = parseRgbColor(actualValue, "actual color");
	if (!actual.ok) return actual;
	const expected = parseRgbColor(expectedValue, "expected color");
	if (!expected.ok) return expected;

	const redDifference = Math.abs(actual.value.r - expected.value.r);
	const greenDifference = Math.abs(actual.value.g - expected.value.g);
	const blueDifference = Math.abs(actual.value.b - expected.value.b);
	const differencePercent =
		mode === "per_channel"
			? (Math.max(redDifference, greenDifference, blueDifference) / 255) * 100
			: (Math.hypot(redDifference, greenDifference, blueDifference) / MAX_TOTAL_DISTANCE) * 100;

	return {
		ok: true,
		value: {
			blue_difference: blueDifference,
			difference_percent: differencePercent,
			green_difference: greenDifference,
			matches: differencePercent <= tolerancePercent,
			red_difference: redDifference,
		},
	};
}

export function validateStaticColor(value: JsonValue, label: string) {
	if (typeof value === "string" && TEMPLATE_REFERENCE.test(value)) {
		return "";
	}
	const parsed = parseRgbColor(value, label);
	return parsed.ok ? "" : parsed.error;
}

export function colorValueToHex(value: JsonValue) {
	const parsed = parseRgbColor(value, "color");
	if (!parsed.ok) return undefined;
	return `#${[parsed.value.r, parsed.value.g, parsed.value.b]
		.map((channel) => channel.toString(16).padStart(2, "0"))
		.join("")
		.toUpperCase()}`;
}

function parseRgbColor(value: JsonValue, label: string): { error: string; ok: false } | { ok: true; value: RgbColor } {
	if (typeof value === "string") {
		const trimmed = value.trim();
		const hex = HEX_COLOR.exec(trimmed);
		if (hex) {
			return {
				ok: true,
				value: {
					r: Number.parseInt(hex[1].slice(0, 2), 16),
					g: Number.parseInt(hex[1].slice(2, 4), 16),
					b: Number.parseInt(hex[1].slice(4, 6), 16),
				},
			};
		}

		const rgb = RGB_COLOR.exec(trimmed);
		if (rgb) {
			return createRgbColor(rgb.slice(1, 4).map(Number), label);
		}
		return {
			error: `${label} must be #RRGGBB, rgb(r, g, b), or an RGB object with r, g, and b channels`,
			ok: false,
		};
	}

	if (isJsonObject(value)) {
		const keys = Object.keys(value);
		if (keys.length !== 3 || !keys.includes("r") || !keys.includes("g") || !keys.includes("b")) {
			return { error: `${label} RGB object must contain exactly r, g, and b channels`, ok: false };
		}
		return createRgbColor([value.r, value.g, value.b], label);
	}

	return {
		error: `${label} must be #RRGGBB, rgb(r, g, b), or an RGB object with r, g, and b channels`,
		ok: false,
	};
}

function createRgbColor(
	channels: unknown[],
	label: string,
): { error: string; ok: false } | { ok: true; value: RgbColor } {
	if (
		!channels.every(
			(channel) => typeof channel === "number" && Number.isInteger(channel) && channel >= 0 && channel <= 255,
		)
	) {
		return { error: `${label} channels must be integers from 0 through 255`, ok: false };
	}
	return {
		ok: true,
		value: { r: channels[0] as number, g: channels[1] as number, b: channels[2] as number },
	};
}

function isJsonObject(value: JsonValue): value is Record<string, JsonValue> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

import type { JsonValue } from "@/lib/types";

const DECIMAL_NUMBER = /^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?$/;

export function conditionValuesEqual(left: JsonValue, right: JsonValue) {
	if (left === right) {
		return true;
	}

	if (typeof left === "number" || typeof right === "number") {
		const leftNumber = conditionNumber(left);
		const rightNumber = conditionNumber(right);
		if (leftNumber !== undefined && rightNumber !== undefined) {
			return leftNumber === rightNumber;
		}
	}

	return String(left) === String(right);
}

function conditionNumber(value: JsonValue) {
	if (typeof value === "number") {
		return Number.isFinite(value) ? value : undefined;
	}

	if (typeof value !== "string" || !DECIMAL_NUMBER.test(value)) {
		return undefined;
	}

	const parsed = Number(value);
	return Number.isFinite(parsed) ? parsed : undefined;
}

import type { JsonValue } from "@/lib/types";
import type { NodeConfigField, NumericConfigContract } from "./node-definition";

const SIGNED_INTEGER_PATTERN = /^-?(?:0|[1-9][0-9]*)$/;
const UNSIGNED_INTEGER_PATTERN = /^(?:0|[1-9][0-9]*)$/;
const SIGNED_FLOAT_PATTERN = /^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?$/;
const UNSIGNED_FLOAT_PATTERN = /^(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?$/;

export const runtimeNumberContract: NumericConfigContract = {
	kind: "float",
	signed: true,
	minimum: "-1.7976931348623157e308",
	maximum: "1.7976931348623157e308",
	minimumInclusive: true,
	maximumInclusive: true,
};

export function validateNumericConfigValue(value: JsonValue, contract: NumericConfigContract) {
	return contract.kind === "integer" ? validateInteger(value, contract) : validateFloat(value, contract);
}

export function numericContractApplies(field: NodeConfigField, config: Record<string, JsonValue>) {
	if (!field.numeric) {
		return false;
	}
	return !field.numericWhen || config[field.numericWhen.key] === field.numericWhen.equals;
}

function validateInteger(value: JsonValue, contract: NumericConfigContract) {
	let parsed: bigint;
	if (typeof value === "number") {
		if (!Number.isSafeInteger(value)) {
			return "must be an exact safe integer when stored as a JSON number; use integer text for larger values";
		}
		parsed = BigInt(value);
	} else if (typeof value === "string") {
		const trimmed = value.trim();
		const pattern = contract.signed ? SIGNED_INTEGER_PATTERN : UNSIGNED_INTEGER_PATTERN;
		if (!pattern.test(trimmed)) {
			return contract.signed ? "must be a whole signed integer" : "must be a whole non-negative integer";
		}
		parsed = BigInt(trimmed);
	} else {
		return "must be an integer";
	}

	const minimum = BigInt(contract.minimum);
	const maximum = BigInt(contract.maximum);
	if (!withinBounds(parsed, minimum, maximum, contract)) {
		return rangeMessage(contract);
	}
	return "";
}

function validateFloat(value: JsonValue, contract: NumericConfigContract) {
	let parsed: number;
	if (typeof value === "number") {
		parsed = value;
	} else if (typeof value === "string") {
		const trimmed = value.trim();
		const pattern = contract.signed ? SIGNED_FLOAT_PATTERN : UNSIGNED_FLOAT_PATTERN;
		if (!pattern.test(trimmed)) {
			return contract.signed ? "must be a signed decimal number" : "must be a non-negative decimal number";
		}
		parsed = Number(trimmed);
	} else {
		return "must be a number";
	}

	if (!Number.isFinite(parsed)) {
		return "must be a finite number";
	}
	const minimum = Number(contract.minimum);
	const maximum = Number(contract.maximum);
	if (!withinBounds(parsed, minimum, maximum, contract)) {
		return rangeMessage(contract);
	}
	return "";
}

function withinBounds<T extends bigint | number>(value: T, minimum: T, maximum: T, contract: NumericConfigContract) {
	const aboveMinimum = contract.minimumInclusive ? value >= minimum : value > minimum;
	const belowMaximum = contract.maximumInclusive ? value <= maximum : value < maximum;
	return aboveMinimum && belowMaximum;
}

function rangeMessage(contract: NumericConfigContract) {
	const minimumOperator = contract.minimumInclusive ? "at least" : "greater than";
	const maximumOperator = contract.maximumInclusive ? "at most" : "less than";
	return `must be ${minimumOperator} ${contract.minimum} and ${maximumOperator} ${contract.maximum}`;
}

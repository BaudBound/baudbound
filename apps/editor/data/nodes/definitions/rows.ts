import type { JsonValue } from "@/lib/types";

export type ConditionRow = {
	id: string;
	invert?: boolean;
	left: string;
	combinator?: string;
	operator: string;
	right: string;
};

export type SwitchCaseRow = {
	id: string;
	name: string;
	value: string;
	expectedValue?: string;
};

export type HeaderRow = {
	id: string;
	name: string;
	value: string;
};

export function createConditionRow(left = "{{status}}", right = "ok", combinator?: string): ConditionRow {
	const row = {
		id: crypto.randomUUID(),
		left,
		operator: "==",
		right,
	};

	return combinator === undefined ? row : { ...row, combinator };
}

export function createSwitchCaseRow(name = "case"): SwitchCaseRow {
	return {
		id: crypto.randomUUID(),
		name,
		value: name,
	};
}

export function createHeaderRow(name = "", value = ""): HeaderRow {
	return {
		id: crypto.randomUUID(),
		name,
		value,
	};
}

export function createSwitchOutputPorts(cases: SwitchCaseRow[]) {
	return cases.map((switchCase, index) => ({
		id: `case-${switchCase.id}`,
		label: switchCase.name.trim() || `case ${index + 1}`,
	}));
}

export function isConditionRow(value: JsonValue): value is ConditionRow {
	return (
		isRecord(value) &&
		typeof value.id === "string" &&
		typeof value.left === "string" &&
		typeof value.operator === "string" &&
		typeof value.right === "string" &&
		(typeof value.invert === "boolean" || value.invert === undefined) &&
		(typeof value.combinator === "string" || value.combinator === undefined)
	);
}

export function isSwitchCaseRow(value: JsonValue): value is SwitchCaseRow {
	return (
		isRecord(value) &&
		typeof value.id === "string" &&
		typeof value.name === "string" &&
		(typeof value.value === "string" || typeof value.expectedValue === "string")
	);
}

export function isHeaderRow(value: JsonValue): value is HeaderRow {
	return isRecord(value) && typeof value.id === "string" && typeof value.name === "string";
}

export function getSwitchCaseRowsFromValue(value: JsonValue | undefined): SwitchCaseRow[] {
	if (!Array.isArray(value)) {
		return [createSwitchCaseRow()];
	}

	const rows = value.filter(isSwitchCaseRow).map((row) => ({
		id: row.id,
		name: row.name,
		value: row.value ?? String(row.expectedValue ?? ""),
	}));

	return rows.length > 0 ? rows : [createSwitchCaseRow()];
}

function isRecord(value: JsonValue): value is Record<string, JsonValue> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

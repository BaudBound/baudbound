import { Database } from "lucide-react";
import {
	getClearedVariableValue,
	getVariableOperationFixedType,
	normalizeVariableOperation,
	type VariableType,
	validateVariableName,
	validateVariableOperationValue,
	variableTypes,
} from "@/data/project/variables";
import type { JsonValue } from "@/lib/types";
import type { SimulationContext } from "@/utils/simulation-types";
import { defineNode, type NodeSimulationApi } from "../../node-definition";
import { variableOperationOptions, variableScopeOptions, variableTypeOptions } from "../options";

export const setVariableNode = defineNode({
	actionType: "runtime.set_variable",
	capabilities: ["runtime.variables"],
	configFields: [
		{ key: "operation", label: "Operation", type: "select", options: variableOperationOptions },
		{ key: "name", label: "Variable name", type: "text" },
		{ key: "scope", label: "Scope", type: "select", options: variableScopeOptions },
		{ key: "valueType", label: "Variable type", type: "select", options: variableTypeOptions },
	],
	defaultConfig: () => ({
		operation: "set",
		name: "status",
		scope: "runtime",
		valueType: "string",
		value: "ok",
		fieldPath: "",
	}),
	description: "Set, increment, append, clear, or edit variable values.",
	group: "actions",
	icon: Database,
	kind: "action",
	label: "Set Variable",
	permission: { name: "set_local_variable", risk: "low" },
	risk: "low",
	runnerType: "set_variable",
	validateConfig: (config) => {
		const name = configString(config.name);
		const nameError = validateVariableName(name);
		const operation = normalizeVariableOperation(configString(config.operation));
		const rawType = configString(config.valueType);
		const fixedType = getVariableOperationFixedType(operation);
		const declaredType = variableTypes.find((type) => type === rawType);
		const valueType = fixedType ?? declaredType;
		const errors = [
			nameError ? `has invalid variable name: ${nameError}` : "",
			valueType ? "" : `has invalid variable type "${rawType || "missing"}".`,
		];

		if (valueType) {
			const valueError = validateVariableOperationValue(
				operation,
				valueType,
				configString(config.value),
				configString(config.fieldPath),
			);
			if (valueError) {
				errors.push(valueError);
			}
		}

		return errors.filter(Boolean);
	},
	simulation: {
		afterExecute: ({ api, context, failed, node }) => {
			if (failed) {
				return [];
			}

			const name = api.getConfigString(node, "name").trim();
			if (!name) {
				return [];
			}

			const result = applyVariableOperation(node.data.config, context, api);
			context.runtimeVariables[name] = result.value;

			return [
				{
					level: "info",
					message: `[Simulation] ${result.message}`,
				},
			];
		},
		describe: ({ api, node }) => [
			{
				level: "info",
				message: `[Simulation] Set Variable (${node.id}) succeeded. Preparing to ${api.getConfigString(node, "operation").replaceAll("_", " ")} ${api.getConfigString(node, "name")}.`,
			},
		],
	},
});

type SetVariableSimulationApi = {
	formatValue: (value: JsonValue) => string;
	getConfigString: NodeSimulationApi["getConfigString"];
	parseJsonValue: (value: string) => JsonValue | undefined;
	resolveJsonCompatibleInput: (value: string, context: SimulationContext) => JsonValue;
	resolveTemplate: (value: string, context: SimulationContext) => JsonValue;
};

function applyVariableOperation(
	config: Record<string, JsonValue>,
	context: SimulationContext,
	api: SetVariableSimulationApi,
) {
	const name = configString(config.name).trim();
	const operation = normalizeVariableOperation(configString(config.operation));
	const type = getVariableOperationFixedType(operation) ?? normalizeVariableType(configString(config.valueType));
	const currentValue = context.runtimeVariables[name];

	if (operation === "increment") {
		const amount = Number(api.resolveTemplate(configString(config.value), context));
		const currentNumber = typeof currentValue === "number" ? currentValue : Number(currentValue);
		const value = (Number.isFinite(currentNumber) ? currentNumber : 0) + (Number.isFinite(amount) ? amount : 0);

		return {
			value,
			message: `Incremented runtime variable "${name}" by ${api.formatValue(amount)} to ${api.formatValue(value)}.`,
		};
	}

	if (operation === "append_list") {
		const item = api.resolveJsonCompatibleInput(configString(config.value), context);
		const value = [...(Array.isArray(currentValue) ? currentValue : []), item];

		return {
			value,
			message: `Appended ${api.formatValue(item)} to list variable "${name}".`,
		};
	}

	if (operation === "set_object_field") {
		const fieldPath = configString(config.fieldPath).trim();
		const fieldValue = api.resolveJsonCompatibleInput(configString(config.value), context);
		const value = setObjectPathValue(currentValue, fieldPath, fieldValue);

		return {
			value,
			message: `Set object field "${name}.${fieldPath}" to ${api.formatValue(fieldValue)}.`,
		};
	}

	if (operation === "clear") {
		const value = resolveVariableInput(getClearedVariableValue(type), type, context, api);

		return {
			value,
			message: `Cleared runtime variable "${name}" to ${api.formatValue(value)}.`,
		};
	}

	const value = resolveVariableInput(configString(config.value), type, context, api);
	return {
		value,
		message: `Set runtime variable "${name}" to ${api.formatValue(value)}.`,
	};
}

function normalizeVariableType(value: string): VariableType {
	return variableTypes.includes(value as VariableType) ? (value as VariableType) : "string";
}

function resolveVariableInput(
	value: string,
	type: VariableType,
	context: SimulationContext,
	api: SetVariableSimulationApi,
): JsonValue {
	const resolved = api.resolveTemplate(value, context);
	if (typeof resolved !== "string") {
		return resolved;
	}

	if (type === "number") {
		const numberValue = Number(resolved);
		return Number.isFinite(numberValue) ? numberValue : resolved;
	}

	if (type === "boolean") {
		return resolved.trim() === "true" ? true : resolved.trim() === "false" ? false : resolved;
	}

	if (type === "list" || type === "object" || type === "duration" || type === "datetime" || type === "http_response") {
		return api.parseJsonValue(resolved) ?? resolved;
	}

	return resolved;
}

function setObjectPathValue(currentValue: JsonValue | undefined, path: string, value: JsonValue): JsonValue {
	const root =
		currentValue && typeof currentValue === "object" && !Array.isArray(currentValue) ? cloneJson(currentValue) : {};
	const parts = parseObjectPath(path);
	let cursor: Record<string, JsonValue> | JsonValue[] = root;

	for (let index = 0; index < parts.length; index += 1) {
		const part = parts[index];
		const isLast = index === parts.length - 1;

		if (isLast) {
			setPathContainerValue(cursor, part, value);
			break;
		}

		const nextPart = parts[index + 1];
		const existing = getPathContainerValue(cursor, part);
		const nextValue =
			existing && typeof existing === "object" ? cloneJson(existing) : typeof nextPart === "number" ? [] : {};

		setPathContainerValue(cursor, part, nextValue);
		cursor = nextValue as Record<string, JsonValue> | JsonValue[];
	}

	return root;
}

function getPathContainerValue(container: Record<string, JsonValue> | JsonValue[], key: string | number) {
	return Array.isArray(container) ? container[Number(key)] : container[String(key)];
}

function setPathContainerValue(
	container: Record<string, JsonValue> | JsonValue[],
	key: string | number,
	value: JsonValue,
) {
	if (Array.isArray(container)) {
		container[Number(key)] = value;
		return;
	}

	container[String(key)] = value;
}

function parseObjectPath(path: string): Array<string | number> {
	return [...path.matchAll(/[A-Za-z_][A-Za-z0-9_]*|\[(0|[1-9][0-9]*)\]/g)].map((match) =>
		match[1] === undefined ? match[0] : Number(match[1]),
	);
}

function cloneJson<T extends JsonValue>(value: T): T {
	return JSON.parse(JSON.stringify(value)) as T;
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

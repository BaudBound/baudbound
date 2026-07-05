import type { JsonValue } from "@/lib/types";
import { defaultInputPort, triggerOutputPort } from "../node-definition";
import { comparisonOperatorOptions } from "./options";
import { isConditionRow } from "./rows";

export const triggerPorts = () => ({ inputs: [], outputs: [triggerOutputPort] });

export const loopPorts = () => ({
	inputs: [defaultInputPort],
	outputs: [
		{ id: "done", label: "done" },
		{ id: "loop", label: "loop" },
	],
});

export function validateLoopBodyDoesNotReturn(
	controlNodeId: string,
	edges: { source: string; sourceHandle?: string | null; target: string }[],
	label: string,
) {
	const loopEdges = edges.filter((edge) => edge.source === controlNodeId && edge.sourceHandle === "loop");

	for (const edge of loopEdges) {
		if (canReachNode(edge.target, controlNodeId, edges)) {
			return [`${controlNodeId} ${label} output must not connect back to ${controlNodeId} input.`];
		}
	}

	return [];
}

function canReachNode(startNodeId: string, targetNodeId: string, edges: { source: string; target: string }[]) {
	const visited = new Set<string>();
	const queue = [startNodeId];

	while (queue.length > 0) {
		const nodeId = queue.shift();
		if (!nodeId || visited.has(nodeId)) {
			continue;
		}

		if (nodeId === targetNodeId) {
			return true;
		}

		visited.add(nodeId);
		for (const edge of edges) {
			if (edge.source === nodeId && edge.target) {
				queue.push(edge.target);
			}
		}
	}

	return false;
}

const comparisonOperators = new Set(comparisonOperatorOptions.map((option) => option.value));
const conditionCombinators = new Set(["and", "or"]);

export function validateConditionRowsConfig(config: Record<string, JsonValue>, label: string) {
	const conditions = config.conditions;
	if (!Array.isArray(conditions) || conditions.length === 0) {
		return [`${label} must have at least one condition.`];
	}

	return conditions.flatMap((condition, index) => {
		const rowLabel = `${label} condition ${index + 1}`;
		if (!isConditionRow(condition)) {
			return [`${rowLabel} is malformed.`];
		}

		const errors: string[] = [];
		if (!condition.left.trim()) {
			errors.push(`${rowLabel} value is required.`);
		}
		if (!comparisonOperators.has(condition.operator)) {
			errors.push(`${rowLabel} uses unknown expression "${condition.operator}".`);
		}
		if (index > 0 && !conditionCombinators.has(condition.combinator ?? "")) {
			errors.push(`${rowLabel} combinator must be AND or OR.`);
		}

		return errors;
	});
}

export const actionAudio = ["action.sound"];
export const actionFile = ["action.file"];
export const actionKeyboard = ["action.keyboard"];
export const actionMouse = ["action.mouse"];
export const actionProcess = ["action.process"];
export const actionWindow = ["action.window"];

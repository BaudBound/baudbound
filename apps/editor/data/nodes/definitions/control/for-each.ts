import { Repeat2 } from "lucide-react";
import { validateVariableName } from "@/data/project/variables";
import { defineNode } from "../../node-definition";
import { loopPorts } from "../shared";
import { requiredConfig } from "../validators";

export const forEachNode = defineNode({
	actionType: "control.for_each",
	capabilities: ["runtime.for_each"],
	configFields: [
		{ key: "items", label: "Items", type: "textarea", usesVariables: true },
		{ key: "itemVariable", label: "Item variable", type: "text" },
		{ key: "indexVariable", label: "Index variable", type: "text" },
	],
	controlType: "for_each",
	defaultConfig: () => ({ items: '["one", "two", "three"]', itemVariable: "item", indexVariable: "index" }),
	description: "Loop through every item in a list.",
	group: "control",
	icon: Repeat2,
	kind: "control",
	label: "For Each",
	ports: loopPorts,
	risk: "low",
	runtimeOutputs: [
		{
			name: "item",
			type: "object",
			description: "Current list item for the active iteration.",
			example: "n-mr3zyt6f-8.item",
		},
		{
			name: "index",
			type: "number",
			description: "Zero-based index for the active iteration.",
			example: "n-mr3zyt6f-8.index",
		},
	],
	validateConfig: (config) =>
		[
			requiredConfig(config, "items", "for-each items"),
			validateVariableConfig(config.itemVariable, "item variable"),
			validateVariableConfig(config.indexVariable, "index variable"),
		].filter(Boolean),
	validateGraph: ({ context, node }) =>
		validateLoopTopology(
			node.id,
			context.edges,
			context.nodes.map((entry) => entry.id),
		),
});

function validateVariableConfig(value: unknown, label: string) {
	const error = validateVariableName(typeof value === "string" ? value : "");
	return error ? `has invalid ${label}: ${error}` : "";
}

function validateLoopTopology(
	loopNodeId: string,
	edges: { source: string; sourceHandle?: string | null; target: string }[],
	nodeIds: string[],
) {
	const nodeIdSet = new Set(nodeIds);
	const loopEdges = edges.filter((edge) => edge.source === loopNodeId && edge.sourceHandle === "loop");

	if (loopEdges.length === 0) {
		return [`${loopNodeId} must connect its loop output back to its input, directly or through loop body nodes.`];
	}

	return loopEdges.some((edge) => canReachNode(edge.target, loopNodeId, edges, nodeIdSet))
		? []
		: [`${loopNodeId} loop output must eventually return to ${loopNodeId} input.`];
}

function canReachNode(
	startNodeId: string,
	targetNodeId: string,
	edges: { source: string; target: string }[],
	nodeIds: ReadonlySet<string>,
) {
	const visited = new Set<string>();
	const queue = [startNodeId];

	while (queue.length > 0) {
		const nodeId = queue.shift();
		if (!nodeId || visited.has(nodeId) || !nodeIds.has(nodeId)) {
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

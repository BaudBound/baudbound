import { Clock } from "lucide-react";
import { defineNode } from "../../node-definition";
import { loopPorts } from "../shared";
import { staticPositiveNumberConfig } from "../validators";

export const loopNode = defineNode({
	actionType: "control.loop",
	capabilities: ["runtime.loop"],
	configFields: [{ key: "count", label: "Repeat count", type: "number", usesVariables: true }],
	controlType: "loop",
	defaultConfig: () => ({ count: "3" }),
	description: "Repeat a block of steps.",
	group: "control",
	icon: Clock,
	kind: "control",
	label: "Loop",
	ports: loopPorts,
	risk: "low",
	validateConfig: (config) => [staticPositiveNumberConfig(config, "count", "loop repeat count")].filter(Boolean),
	validateGraph: ({ context, node }) =>
		validateLoopTopology(
			node.id,
			context.edges,
			context.nodes.map((entry) => entry.id),
		),
});

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

import { defaultInputPort, triggerOutputPort } from "../node-definition";

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

export const actionAudio = ["action.sound"];
export const actionFile = ["action.file"];
export const actionKeyboard = ["action.keyboard"];
export const actionMouse = ["action.mouse"];
export const actionProcess = ["action.process"];
export const actionWindow = ["action.window"];

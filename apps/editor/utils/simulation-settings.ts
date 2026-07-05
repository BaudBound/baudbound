import type { Node } from "@xyflow/react";
import type { ScriptNodeData, SimulationSettings } from "@/lib/types";

export function getSimulationStepDelay(speed: SimulationSettings["speed"]) {
	if (speed === "slow") {
		return 520;
	}

	if (speed === "fast") {
		return 70;
	}

	if (speed === "instant") {
		return 0;
	}

	return 220;
}

export function getSimulationTriggers(nodes: Node<ScriptNodeData>[]) {
	return nodes.filter((node) => node.data.kind === "trigger");
}

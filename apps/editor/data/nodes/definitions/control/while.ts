import { RotateCw } from "lucide-react";
import { defineNode } from "../../node-definition";
import { createConditionRow } from "../rows";
import { validateConditionRowsConfig, validateLoopBodyDoesNotReturn } from "../shared";

export const whileNode = defineNode({
	actionType: "control.while",
	capabilities: ["runtime.while"],
	controlType: "while",
	defaultConfig: () => ({ conditions: [createConditionRow("{{status}}", "running")] }),
	description: "Repeat a branch while conditions pass.",
	group: "control",
	icon: RotateCw,
	kind: "control",
	label: "While",
	portPolicy: { kind: "fixed", inputs: ["input"], outputs: ["done", "loop"] },
	risk: "low",
	validateConfig: (config) => validateConditionRowsConfig(config, "while"),
	validateGraph: ({ context, node }) => validateLoopBodyDoesNotReturn(node.id, context.edges, "loop"),
});

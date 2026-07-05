import { Clock } from "lucide-react";
import { defineNode } from "../../node-definition";
import { loopPorts, validateLoopBodyDoesNotReturn } from "../shared";
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
	validateGraph: ({ context, node }) => validateLoopBodyDoesNotReturn(node.id, context.edges, "loop"),
});

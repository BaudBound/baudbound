import { Clock } from "lucide-react";
import { defineNode } from "../../node-definition";
import { validateLoopBodyDoesNotReturn } from "../shared";
import { staticPositiveNumberConfig } from "../validators";

export const loopNode = defineNode({
	actionType: "control.loop",
	capabilities: ["runtime.loop"],
	configFields: [
		{
			key: "count",
			label: "Repeat count",
			type: "number",
			usesVariables: true,
			numeric: {
				kind: "integer",
				signed: false,
				minimum: "1",
				maximum: "18446744073709551615",
				minimumInclusive: true,
				maximumInclusive: true,
			},
		},
	],
	controlType: "loop",
	defaultConfig: () => ({ count: "3" }),
	description: "Repeat a block of steps.",
	group: "control",
	icon: Clock,
	kind: "control",
	label: "Loop",
	portPolicy: { kind: "fixed", inputs: ["input"], outputs: ["done", "loop"] },
	risk: "low",
	validateConfig: (config) => [staticPositiveNumberConfig(config, "count", "loop repeat count")].filter(Boolean),
	validateGraph: ({ context, node }) => validateLoopBodyDoesNotReturn(node.id, context.edges, "loop"),
});

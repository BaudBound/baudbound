import { Clock } from "lucide-react";
import { defineNode } from "../../node-definition";
import { loopPorts } from "../shared";

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
});

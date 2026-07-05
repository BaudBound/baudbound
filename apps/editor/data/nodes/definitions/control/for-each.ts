import { Repeat2 } from "lucide-react";
import { defineNode } from "../../node-definition";
import { loopPorts } from "../shared";

export const forEachNode = defineNode({
	actionType: "control.for_each",
	capabilities: ["runtime.foreach"],
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
});

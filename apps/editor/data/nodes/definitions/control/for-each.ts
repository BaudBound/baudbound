import { Repeat2 } from "lucide-react";
import { validateVariableName } from "@/data/project/variables";
import { defineNode } from "../../node-definition";
import { loopPorts, validateLoopBodyDoesNotReturn } from "../shared";
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
	validateGraph: ({ context, node }) => validateLoopBodyDoesNotReturn(node.id, context.edges, "loop"),
});

function validateVariableConfig(value: unknown, label: string) {
	const error = validateVariableName(typeof value === "string" ? value : "");
	return error ? `has invalid ${label}: ${error}` : "";
}

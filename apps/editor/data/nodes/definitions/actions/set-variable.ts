import { Database } from "lucide-react";
import { defineNode } from "../../node-definition";
import { variableOperationOptions, variableScopeOptions, variableTypeOptions } from "../options";

export const setVariableNode = defineNode({
	actionType: "runtime.set_variable",
	capabilities: ["runtime.variables"],
	configFields: [
		{ key: "operation", label: "Operation", type: "select", options: variableOperationOptions },
		{ key: "name", label: "Variable name", type: "text" },
		{ key: "scope", label: "Scope", type: "select", options: variableScopeOptions },
		{ key: "valueType", label: "Variable type", type: "select", options: variableTypeOptions },
	],
	defaultConfig: () => ({
		operation: "set",
		name: "status",
		scope: "runtime",
		valueType: "string",
		value: "ok",
		fieldPath: "",
	}),
	description: "Set, increment, append, clear, or edit variable values.",
	group: "actions",
	icon: Database,
	kind: "action",
	label: "Set Variable",
	permission: { name: "set_local_variable", risk: "low" },
	risk: "low",
	simulation: {
		describe: ({ api, node }) => [
			{
				level: "info",
				message: `[Simulation] Set Variable (${node.id}) succeeded. Preparing to ${api.getConfigString(node, "operation").replaceAll("_", " ")} ${api.getConfigString(node, "name")}.`,
			},
		],
	},
});

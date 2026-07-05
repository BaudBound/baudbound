import { Calculator } from "lucide-react";
import { evaluateCalculationExpression } from "@/data/project/calculation";
import type { NodeExecutionResult } from "@/utils/simulation-types";
import { defineNode } from "../../node-definition";
import { fallible } from "../runtime-outputs";

export const calculateNode = defineNode({
	actionType: "action.calculate",
	capabilities: ["runtime.calculate"],
	configFields: [{ key: "expression", label: "Expression", type: "textarea", usesVariables: true }],
	defaultConfig: () => ({ expression: "1 + 1" }),
	description: "Calculate a numeric expression and expose the result.",
	fallible: true,
	group: "actions",
	icon: Calculator,
	kind: "action",
	label: "Calculate",
	permission: { name: "calculate", risk: "low" },
	risk: "low",
	runtimeOutputs: fallible([
		{
			name: "result",
			type: "number",
			description: "Numeric result of the evaluated expression.",
			example: "n-mr3zyt6f-18.result",
		},
	]),
	runnerType: "calculate",
	simulation: {
		createOutput: ({ api, context, node }): NodeExecutionResult => {
			const expression = String(api.resolveTemplate(api.getConfigString(node, "expression"), context));
			const result = evaluateCalculationExpression(expression);
			if (!result.ok) {
				return {
					failed: true,
					outputData: {
						error: api.createError(result.message, "CALCULATION_FAILED", "validation", { expression }),
					},
				};
			}
			return { failed: false, outputData: { result: result.value } };
		},
		describe: ({ api, context, node }) => {
			const output = context.nodeOutputs[node.id];
			const expression = api.formatValue(api.resolveTemplate(api.getConfigString(node, "expression"), context));
			return [
				{
					level: "info",
					message:
						typeof output?.result === "number"
							? `[Simulation] Calculated ${expression} = ${output.result}.`
							: `[Simulation] Would calculate ${expression}.`,
				},
			];
		},
	},
});

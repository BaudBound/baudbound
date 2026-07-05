import { Clock } from "lucide-react";
import { defineNode } from "../../node-definition";
import { timeUnitOptions } from "../options";
import { staticPositiveNumberConfig } from "../validators";

export const delayNode = defineNode({
	actionType: "action.delay",
	capabilities: ["action.delay"],
	configFields: [
		{ key: "amount", label: "Amount", type: "number", usesVariables: true },
		{ key: "unit", label: "Unit", type: "select", options: timeUnitOptions },
	],
	defaultConfig: () => ({ amount: "1", unit: "seconds" }),
	description: "Pause execution for a duration.",
	group: "actions",
	icon: Clock,
	kind: "action",
	label: "Delay",
	permission: { name: "delay", risk: "low" },
	risk: "low",
	runnerType: "delay",
	validateConfig: (config) => [staticPositiveNumberConfig(config, "amount", "delay amount")].filter(Boolean),
	simulation: {
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Delay (${node.id}) succeeded. Waited ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "amount"), context))} ${api.getConfigString(node, "unit")}.`,
			},
		],
	},
});

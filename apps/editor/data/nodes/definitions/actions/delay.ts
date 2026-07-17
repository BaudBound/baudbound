import { Clock } from "lucide-react";
import type { JsonValue } from "@/lib/types";
import { defineNode } from "../../node-definition";
import { timeUnitOptions } from "../options";
import { staticPositiveDurationConfig, validateDurationValue } from "../validators";

export const delayNode = defineNode({
	actionType: "action.delay",
	capabilities: ["action.delay"],
	configFields: [
		{
			key: "amount",
			label: "Amount",
			type: "number",
			usesVariables: true,
			numeric: {
				kind: "float",
				signed: false,
				minimum: "0",
				maximum: "1.7976931348623157e308",
				minimumInclusive: false,
				maximumInclusive: true,
			},
		},
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
	validateConfig: (config) =>
		[staticPositiveDurationConfig(config, "amount", "unit", "delay duration", true)].filter(Boolean),
	simulation: {
		createOutput: ({ api, context, node }) => {
			const amount = api.resolveTemplate(api.getConfigString(node, "amount"), context);
			const unit = api.getConfigString(node, "unit");
			const error = validateDurationValue(amount, unit, "delay duration");

			if (!error) {
				return { failed: false, outputData: {} };
			}

			const outputData: Record<string, JsonValue> = {
				error: api.createError(error, "DELAY_DURATION_INVALID", "validation", {
					amount,
					unit,
				}),
			};
			return { failed: true, outputData };
		},
		describe: ({ api, context, failed, node }) => [
			failed
				? {
						level: "error",
						message: `[Simulation] Delay (${node.id}) failed because its duration is invalid.`,
					}
				: {
						level: "info",
						message: `[Simulation] Delay (${node.id}) succeeded. Waited ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "amount"), context))} ${api.getConfigString(node, "unit")}.`,
					},
		],
	},
});

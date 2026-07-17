import { Clock } from "lucide-react";
import { defineNode } from "../../node-definition";
import { timeUnitOptions } from "../options";
import { staticPositiveDurationConfig } from "../validators";

export const scheduleTriggerNode = defineNode({
	actionType: "trigger.schedule",
	capabilities: ["trigger.schedule"],
	configFields: [
		{
			key: "every",
			label: "Every",
			type: "number",
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
	defaultConfig: () => ({ every: "5", unit: "minutes" }),
	description: "Run on a time schedule.",
	group: "triggers",
	icon: Clock,
	kind: "trigger",
	label: "Schedule",
	risk: "low",
	runnerType: "schedule",
	validateConfig: (config) =>
		[staticPositiveDurationConfig(config, "every", "unit", "schedule interval")].filter(Boolean),
});

import { Clock } from "lucide-react";
import { defineNode } from "../../node-definition";
import { timeUnitOptions } from "../options";
import { triggerPorts } from "../shared";
import { staticPositiveDurationConfig } from "../validators";

export const scheduleTriggerNode = defineNode({
	actionType: "trigger.schedule",
	capabilities: ["trigger.schedule"],
	configFields: [
		{ key: "every", label: "Every", type: "number" },
		{ key: "unit", label: "Unit", type: "select", options: timeUnitOptions },
	],
	defaultConfig: () => ({ every: "5", unit: "minutes" }),
	description: "Run on a time schedule.",
	group: "triggers",
	icon: Clock,
	kind: "trigger",
	label: "Schedule",
	ports: triggerPorts,
	risk: "low",
	runnerType: "schedule",
	validateConfig: (config) =>
		[staticPositiveDurationConfig(config, "every", "unit", "schedule interval")].filter(Boolean),
});

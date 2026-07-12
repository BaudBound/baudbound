import { Play } from "lucide-react";
import { defineNode } from "../../node-definition";

export const manualTriggerNode = defineNode({
	actionType: "trigger.manual",
	capabilities: ["trigger.manual"],
	description: "Start the script manually.",
	group: "triggers",
	icon: Play,
	kind: "trigger",
	label: "Manual",
	risk: "low",
	runnerType: "manual",
});

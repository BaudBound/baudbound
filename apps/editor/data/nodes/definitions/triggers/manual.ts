import { Play } from "lucide-react";
import { defineNode } from "../../node-definition";
import { triggerPorts } from "../shared";

export const manualTriggerNode = defineNode({
	actionType: "trigger.manual",
	capabilities: ["trigger.manual"],
	description: "Start the script manually.",
	group: "triggers",
	icon: Play,
	kind: "trigger",
	label: "Manual",
	ports: triggerPorts,
	risk: "low",
	runnerType: "manual",
});

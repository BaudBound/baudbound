import { Power } from "lucide-react";
import { defineNode } from "../../node-definition";

export const startupTriggerNode = defineNode({
	actionType: "trigger.startup",
	capabilities: ["trigger.startup"],
	description: "Start when the computer or runner session starts.",
	group: "triggers",
	icon: Power,
	kind: "trigger",
	label: "Startup",
	permission: { name: "startup_trigger", risk: "high" },
	risk: "high",
	runtimeOutputs: [
		{
			name: "timestamp",
			type: "string",
			description: "Runner timestamp when startup triggered the script.",
			example: "n-mr3zyt6f-6.timestamp",
		},
		{
			name: "reason",
			type: "string",
			description: "Startup reason reported by the runner.",
			example: "n-mr3zyt6f-6.reason",
		},
	],
	runnerType: "startup",
	simulation: {
		createOutput: ({ context }) => ({
			failed: false,
			outputData: {
				timestamp: new Date().toISOString(),
				reason: context.triggerPayload.reason || "runner_startup",
			},
		}),
	},
});

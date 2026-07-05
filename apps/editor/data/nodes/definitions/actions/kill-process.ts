import { Skull } from "lucide-react";
import { defineNode } from "../../node-definition";
import { killProcessMatchModeOptions } from "../options";
import { fallible } from "../runtime-outputs";
import { actionProcess } from "../shared";

export const killProcessNode = defineNode({
	actionType: "action.process.kill",
	capabilities: actionProcess,
	configFields: [
		{ key: "matchMode", label: "Match by", type: "select", options: killProcessMatchModeOptions },
		{ key: "target", label: "Target", type: "text", usesVariables: true },
	],
	defaultConfig: () => ({ matchMode: "process_name", target: "app.exe" }),
	description: "Terminate a target process.",
	fallible: true,
	group: "actions",
	icon: Skull,
	kind: "action",
	label: "Kill Process",
	permission: { name: "kill_process", risk: "high" },
	risk: "high",
	runtimeOutputs: fallible([
		{
			name: "process_id",
			type: "process_id",
			description: "Terminated process identifier.",
			example: "n-mr3zyt6f-17.process_id",
		},
		{
			name: "process_name",
			type: "string",
			description: "Terminated process name.",
			example: "n-mr3zyt6f-17.process_name",
		},
	]),
	runnerType: "kill_process",
	simulation: {
		createOutput: ({ api, context, node }) => {
			const processId = getSimulatedProcessId(
				api.getConfigString(node, "matchMode"),
				api.resolveTemplate(api.getConfigString(node, "target"), context),
			);

			return {
				failed: false,
				outputData: {
					process_id: processId,
					process_name:
						api.getConfigString(node, "matchMode") === "pid"
							? `pid:${processId}`
							: String(api.resolveTemplate(api.getConfigString(node, "target"), context)) || "app.exe",
				},
			};
		},
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Kill Process (${node.id}) succeeded. Would terminate ${api.getConfigString(node, "matchMode")} ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "target"), context))}.`,
			},
		],
	},
});

function getSimulatedProcessId(matchMode: string, target: unknown) {
	if (matchMode !== "pid") {
		return 4242;
	}

	const processId = Number(target);
	return Number.isFinite(processId) && processId >= 0 ? Math.trunc(processId) : 4242;
}

import { Code } from "lucide-react";
import { defineNode } from "../../node-definition";
import { fallible } from "../runtime-outputs";
import { actionProcess } from "../shared";

export const runProcessNode = defineNode({
	actionType: "action.process.run",
	capabilities: actionProcess,
	configFields: [
		{ key: "executable", label: "Executable", type: "text", usesVariables: true },
		{ key: "arguments", label: "Arguments", type: "textarea", usesVariables: true },
		{ key: "workingDirectory", label: "Working directory", type: "text", usesVariables: true },
	],
	defaultConfig: () => ({ executable: "ffmpeg", arguments: "-version", workingDirectory: "" }),
	description: "Start an executable with arguments.",
	fallible: true,
	group: "actions",
	icon: Code,
	kind: "action",
	label: "Run Process",
	permission: { name: "run_process", risk: "high" },
	risk: "high",
	runtimeOutputs: fallible([
		{
			name: "process_id",
			type: "process_id",
			description: "Started process identifier when available.",
			example: "n-mr3zyt6f-15.process_id",
		},
	]),
	runnerType: "run_process",
	simulation: {
		createOutput: () => ({ failed: false, outputData: { process_id: 4242 } }),
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Run Process (${node.id}) succeeded. Would run ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "executable"), context))} ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "arguments"), context))}.`,
			},
		],
	},
});

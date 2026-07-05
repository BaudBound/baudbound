import { Play } from "lucide-react";
import { defineNode } from "../../node-definition";
import { fallible } from "../runtime-outputs";

export const subScriptNode = defineNode({
	actionType: "action.script.run",
	capabilities: ["action.script"],
	configFields: [{ key: "script", label: "Script", type: "text", usesVariables: true }],
	defaultConfig: () => ({ script: "other-script" }),
	description: "Run another local script through its manual trigger.",
	fallible: true,
	group: "actions",
	icon: Play,
	kind: "action",
	label: "Sub-script",
	permission: { name: "run_subscript", risk: "medium" },
	risk: "medium",
	runtimeOutputs: fallible([
		{ name: "status", type: "string", description: "Sub-script run status.", example: "n-mr3zyt6f-18.status" },
		{
			name: "exit_code",
			type: "exit_code",
			description: "Sub-script exit code when available.",
			example: "n-mr3zyt6f-18.exit_code",
		},
	]),
	runnerType: "run_script",
	sanitizeConfig: ({ arguments: _arguments, ...config }) => config,
	simulation: {
		createOutput: () => ({ failed: false, outputData: { status: "completed", exit_code: 0 } }),
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Sub-script (${node.id}) succeeded. Would run the manual trigger in sub-script ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "script"), context))}.`,
			},
		],
	},
});

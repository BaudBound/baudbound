import { Terminal } from "lucide-react";
import { defineNode } from "../../node-definition";
import { fallible } from "../runtime-outputs";
import { actionProcess } from "../shared";
import { requiredConfig } from "../validators";

export const shellCommandNode = defineNode({
	actionType: "action.shell",
	capabilities: actionProcess,
	configFields: [{ key: "command", label: "Command", type: "textarea", usesVariables: true }],
	defaultConfig: () => ({ command: "echo blocked-by-default" }),
	description: "Run a shell command.",
	fallible: true,
	group: "actions",
	icon: Terminal,
	kind: "action",
	label: "Shell Command",
	permission: { name: "run_shell_command", risk: "dangerous" },
	risk: "dangerous",
	runtimeOutputs: fallible([
		{
			name: "exit_code",
			type: "exit_code",
			description: "Shell process exit code.",
			example: "n-mr3zyt6f-20.exit_code",
		},
		{ name: "stdout", type: "string", description: "Captured standard output.", example: "n-mr3zyt6f-20.stdout" },
		{ name: "stderr", type: "string", description: "Captured standard error.", example: "n-mr3zyt6f-20.stderr" },
	]),
	runnerType: "run_shell_command",
	validateConfig: (config) => [requiredConfig(config, "command", "shell command")].filter(Boolean),
	simulation: {
		createOutput: () => ({ failed: false, outputData: { exit_code: 0, stdout: "Simulated shell output", stderr: "" } }),
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Shell Command (${node.id}) succeeded. Would run shell command ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "command"), context))}.`,
			},
		],
	},
});

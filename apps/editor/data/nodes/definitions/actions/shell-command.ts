import { Terminal } from "lucide-react";
import { defineNode } from "../../node-definition";
import { fallible } from "../runtime-outputs";
import { actionProcess } from "../shared";
import { requiredConfig, staticOptionalNumberRangeConfig } from "../validators";

export const shellCommandNode = defineNode({
	actionType: "action.shell",
	capabilities: actionProcess,
	configFields: [
		{ key: "command", label: "Command", type: "textarea", usesVariables: true },
		{
			key: "timeoutSeconds",
			label: "Timeout seconds",
			type: "number",
			usesVariables: true,
			required: false,
			numeric: {
				kind: "float",
				signed: false,
				minimum: "1",
				maximum: "86400",
				minimumInclusive: true,
				maximumInclusive: true,
			},
		},
	],
	defaultConfig: () => ({ command: "echo blocked-by-default", timeoutSeconds: "300" }),
	description: "Run a command string through the operating system shell and capture its result and output.",
	fallible: true,
	group: "actions",
	icon: Terminal,
	kind: "action",
	label: "Shell Command",
	permission: { name: "run_shell_command", risk: "dangerous" },
	risk: "dangerous",
	runtimeOutputs: fallible([
		{
			name: "process_id",
			type: "process_id",
			description: "Started shell process identifier.",
			example: "n-mr3zyt6f-20.process_id",
		},
		{
			name: "exit_code",
			type: "exit_code",
			description: "Shell process exit code.",
			example: "n-mr3zyt6f-20.exit_code",
		},
		{
			name: "success",
			type: "boolean",
			description: "Whether the shell exited with code 0.",
			example: "n-mr3zyt6f-20.success",
		},
		{ name: "stdout", type: "string", description: "Captured standard output.", example: "n-mr3zyt6f-20.stdout" },
		{ name: "stderr", type: "string", description: "Captured standard error.", example: "n-mr3zyt6f-20.stderr" },
	]),
	runnerType: "run_shell_command",
	validateConfig: (config) =>
		[
			requiredConfig(config, "command", "shell command"),
			staticOptionalNumberRangeConfig(config, "timeoutSeconds", "shell timeout", 1, 86_400),
		].filter(Boolean),
	simulation: {
		createOutput: () => ({
			failed: false,
			outputData: { process_id: 4242, exit_code: 0, success: true, stdout: "Simulated shell output", stderr: "" },
		}),
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Shell Command (${node.id}) succeeded. Would run shell command ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "command"), context))}.`,
			},
		],
	},
});

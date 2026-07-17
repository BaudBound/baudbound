import { Code } from "lucide-react";
import { defineNode } from "../../node-definition";
import { fallible } from "../runtime-outputs";
import { actionProcess } from "../shared";
import { requiredConfig, staticOptionalNumberRangeConfig } from "../validators";

export const runProcessNode = defineNode({
	actionType: "action.process.run",
	capabilities: actionProcess,
	configFields: [
		{ key: "executable", label: "Executable", type: "text", usesVariables: true },
		{ key: "arguments", label: "Arguments", type: "textarea", usesVariables: true },
		{ key: "workingDirectory", label: "Working directory", type: "text", usesVariables: true },
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
	defaultConfig: () => ({ executable: "ffmpeg", arguments: "-version", workingDirectory: "", timeoutSeconds: "300" }),
	description:
		"Execute a program, wait for it to finish, and capture its exit code, standard output, and standard error.",
	fallible: true,
	group: "actions",
	icon: Code,
	kind: "action",
	label: "Run Process",
	permission: { name: "run_process", risk: "dangerous" },
	risk: "dangerous",
	runtimeOutputs: fallible([
		{
			name: "process_id",
			type: "process_id",
			description: "Started process identifier.",
			example: "n-mr3zyt6f-15.process_id",
		},
		{
			name: "exit_code",
			type: "exit_code",
			description: "Process exit code, or null when the operating system did not provide one.",
			example: "n-mr3zyt6f-15.exit_code",
		},
		{
			name: "success",
			type: "boolean",
			description: "Whether the process exited with code 0.",
			example: "n-mr3zyt6f-15.success",
		},
		{
			name: "stdout",
			type: "string",
			description: "Captured standard output.",
			example: "n-mr3zyt6f-15.stdout",
		},
		{
			name: "stderr",
			type: "string",
			description: "Captured standard error.",
			example: "n-mr3zyt6f-15.stderr",
		},
	]),
	runnerType: "run_process",
	validateConfig: (config) =>
		[
			requiredConfig(config, "executable", "process executable"),
			staticOptionalNumberRangeConfig(config, "timeoutSeconds", "process timeout", 1, 86_400),
		].filter(Boolean),
	simulation: {
		createOutput: () => ({
			failed: false,
			outputData: { process_id: 4242, exit_code: 0, success: true, stdout: "Simulated process output", stderr: "" },
		}),
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Run Process (${node.id}) succeeded. Would run ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "executable"), context))} ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "arguments"), context))}.`,
			},
		],
	},
});

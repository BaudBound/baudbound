import { AppWindow } from "lucide-react";
import { defineNode } from "../../node-definition";
import { processMatchModeOptions } from "../options";
import { requiredConfig, windowsDesktopOnlyConfigValue } from "../validators";

export const processStartedTriggerNode = defineNode({
	actionType: "trigger.process_started",
	capabilities: ["trigger.process_started"],
	configFields: [
		{ key: "matchMode", label: "Match by", type: "select", options: processMatchModeOptions },
		{
			key: "target",
			label: "Target",
			type: "text",
			help: "Process name, executable path, or window title depending on Match by.",
		},
	],
	defaultConfig: () => ({ matchMode: "process_name", target: "app.exe" }),
	description: "Start when a configured app or process starts.",
	group: "triggers",
	icon: AppWindow,
	kind: "trigger",
	label: "App / Process Started",
	risk: "medium",
	runtimeOutputs: [
		{
			name: "process_name",
			type: "string",
			description: "Started process name.",
			example: "n-mr3zyt6f-7.process_name",
		},
		{
			name: "process_id",
			type: "process_id",
			description: "Started process identifier.",
			example: "n-mr3zyt6f-7.process_id",
		},
		{
			name: "executable_path",
			type: "file_path",
			description: "Executable path when the runner exposes it.",
			example: "n-mr3zyt6f-7.executable_path",
		},
		{
			name: "window_title",
			type: "string",
			description: "Initial window title when available.",
			example: "n-mr3zyt6f-7.window_title",
		},
		{
			name: "timestamp",
			type: "string",
			description: "Runner timestamp when the process was detected.",
			example: "n-mr3zyt6f-7.timestamp",
		},
	],
	runnerType: "process_started",
	validateConfig: (config) => [requiredConfig(config, "target", "process start target")].filter(Boolean),
	validateTargetRuntime: ({ config, targetRuntime }) =>
		[
			windowsDesktopOnlyConfigValue(
				config,
				"matchMode",
				"window_title",
				targetRuntime,
				"Window-title process matching",
			),
		].filter(Boolean),
	simulation: {
		createOutput: ({ api, context, node }) => ({
			failed: false,
			outputData: {
				process_name: context.triggerPayload.processName || api.getConfigString(node, "target") || "app.exe",
				process_id: Number(context.triggerPayload.processId) || 4244,
				executable_path: context.triggerPayload.executablePath || "",
				window_title: context.triggerPayload.windowTitle || "",
				timestamp: new Date().toISOString(),
			},
		}),
	},
});

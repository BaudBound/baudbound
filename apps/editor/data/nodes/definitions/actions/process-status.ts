import { BadgeInfo } from "lucide-react";
import { defineNode } from "../../node-definition";
import { processMatchModeOptions } from "../options";
import { fallible, processStatusRuntimeOutputs } from "../runtime-outputs";
import { actionProcess } from "../shared";
import { requiredConfig, windowsDesktopOnlyConfigValue } from "../validators";

export const processStatusNode = defineNode({
	actionType: "action.process.status",
	capabilities: actionProcess,
	configFields: [
		{ key: "matchMode", label: "Match by", type: "select", options: processMatchModeOptions },
		{ key: "target", label: "Target", type: "text", usesVariables: true },
	],
	defaultConfig: () => ({ matchMode: "process_name", target: "app.exe" }),
	description: "Read process status and state.",
	fallible: true,
	group: "actions",
	icon: BadgeInfo,
	kind: "action",
	label: "Process Status",
	permission: { name: "process_query", risk: "medium" },
	risk: "medium",
	runtimeOutputs: fallible(processStatusRuntimeOutputs()),
	runnerType: "process_status",
	validateConfig: (config) => [requiredConfig(config, "target", "process target")].filter(Boolean),
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
				running: true,
				state: "running",
				process_id: 4242,
				process_name: String(api.resolveTemplate(api.getConfigString(node, "target"), context)) || "app.exe",
			},
		}),
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Process Status (${node.id}) succeeded. Would check ${api.getConfigString(node, "matchMode")} ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "target"), context))}; simulated state is running.`,
			},
		],
	},
});

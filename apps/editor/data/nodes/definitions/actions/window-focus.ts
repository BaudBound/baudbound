import { AppWindow } from "lucide-react";
import { defineNode } from "../../node-definition";
import { processMatchModeOptions } from "../options";
import { actionWindow } from "../shared";
import { requiredConfig } from "../validators";

export const windowFocusNode = defineNode({
	actionType: "action.window.focus",
	capabilities: actionWindow,
	configFields: [
		{ key: "matchMode", label: "Match by", type: "select", options: processMatchModeOptions },
		{ key: "target", label: "Target", type: "text", usesVariables: true },
	],
	defaultConfig: () => ({ matchMode: "window_title", target: "Untitled" }),
	description: "Focus a target window.",
	desktopOnly: true,
	fallible: true,
	group: "actions",
	icon: AppWindow,
	kind: "action",
	label: "Window Focus",
	permission: { name: "window_focus", risk: "high" },
	risk: "high",
	supportedTargetRuntimes: ["Windows Desktop"],
	runnerType: "focus_window",
	validateConfig: (config) => [requiredConfig(config, "target", "window focus target")].filter(Boolean),
	simulation: {
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Window Focus (${node.id}) succeeded. Would focus window using ${api.getConfigString(node, "matchMode")} ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "target"), context))}.`,
			},
		],
	},
});

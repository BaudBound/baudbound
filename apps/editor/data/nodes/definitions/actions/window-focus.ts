import { AppWindow } from "lucide-react";
import { defineNode } from "../../node-definition";
import { processMatchModeOptions } from "../options";
import { actionWindow } from "../shared";

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
	permission: { name: "window_focus", risk: "medium" },
	risk: "medium",
	runnerType: "focus_window",
	simulation: {
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Window Focus (${node.id}) succeeded. Would focus window using ${api.getConfigString(node, "matchMode")} ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "target"), context))}.`,
			},
		],
	},
});

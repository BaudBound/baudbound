import { MousePointer } from "lucide-react";
import { defineNode } from "../../node-definition";
import { mouseButtonOptions, mouseClickTypeOptions } from "../options";
import { actionMouse } from "../shared";
import { requiredConfig } from "../validators";

export const mouseClickNode = defineNode({
	actionType: "action.mouse",
	capabilities: actionMouse,
	configFields: [
		{ key: "button", label: "Button", type: "select", options: mouseButtonOptions },
		{ key: "clickType", label: "Click", type: "select", options: mouseClickTypeOptions },
	],
	defaultConfig: () => ({ button: "left", clickType: "single" }),
	description: "Move or click the mouse.",
	desktopOnly: true,
	fallible: true,
	group: "actions",
	icon: MousePointer,
	kind: "action",
	label: "Mouse Click",
	permission: { name: "mouse_control", risk: "high" },
	risk: "high",
	runnerType: "mouse_click",
	validateConfig: (config) =>
		[requiredConfig(config, "button", "mouse button"), requiredConfig(config, "clickType", "mouse click type")].filter(
			Boolean,
		),
	validateTargetRuntime: ({ config, targetRuntime }) => {
		if (targetRuntime !== "macOS Desktop") {
			return [];
		}

		const button = typeof config.button === "string" ? config.button.trim().toLowerCase() : "";
		return button === "back" || button === "forward"
			? [`uses the ${button} mouse button, which does not have a native macOS backend.`]
			: [];
	},
	simulation: {
		describe: ({ api, node }) => [
			{
				level: "info",
				message: `[Simulation] Mouse Click (${node.id}) succeeded. Would ${api.getConfigString(node, "clickType")} ${api.getConfigString(node, "button")} mouse button.`,
			},
		],
	},
});

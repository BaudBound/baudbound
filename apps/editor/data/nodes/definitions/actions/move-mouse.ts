import { MousePointer2 } from "lucide-react";
import { defineNode } from "../../node-definition";
import { actionMouse } from "../shared";
import { configString, staticNonNegativeNumberConfig, staticNumberConfig } from "../validators";

export const moveMouseNode = defineNode({
	actionType: "action.mouse.move",
	capabilities: actionMouse,
	configFields: [
		{ key: "x", label: "X", type: "number", usesVariables: true },
		{ key: "y", label: "Y", type: "number", usesVariables: true },
		{ key: "relative", label: "Relative move", type: "switch" },
	],
	defaultConfig: () => ({ x: "100", y: "100", relative: false }),
	description: "Move the mouse to absolute or relative coordinates.",
	desktopOnly: true,
	fallible: true,
	group: "actions",
	icon: MousePointer2,
	kind: "action",
	label: "Move Mouse",
	permission: { name: "mouse_control", risk: "high" },
	risk: "high",
	runnerType: "move_mouse",
	validateConfig: (config) => {
		const relative = configString(config, "relative") === "true" || config.relative === true;
		return [
			relative
				? staticNumberConfig(config, "x", "mouse X offset")
				: staticNonNegativeNumberConfig(config, "x", "mouse X coordinate"),
			relative
				? staticNumberConfig(config, "y", "mouse Y offset")
				: staticNonNegativeNumberConfig(config, "y", "mouse Y coordinate"),
		].filter(Boolean);
	},
	simulation: {
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Move Mouse (${node.id}) succeeded. Would move mouse ${api.getConfigString(node, "relative") === "true" ? "relatively by" : "to"} x=${api.formatValue(api.resolveTemplate(api.getConfigString(node, "x"), context))}, y=${api.formatValue(api.resolveTemplate(api.getConfigString(node, "y"), context))}.`,
			},
		],
	},
});

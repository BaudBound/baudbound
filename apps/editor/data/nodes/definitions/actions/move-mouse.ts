import { MousePointer2 } from "lucide-react";
import { defineNode } from "../../node-definition";
import { actionMouse } from "../shared";

export const moveMouseNode = defineNode({
	actionType: "action.mouse.move",
	capabilities: actionMouse,
	configFields: [
		{
			key: "x",
			label: "X",
			type: "number",
			usesVariables: true,
			numeric: {
				kind: "integer",
				signed: true,
				minimum: "-2147483648",
				maximum: "2147483647",
				minimumInclusive: true,
				maximumInclusive: true,
			},
		},
		{
			key: "y",
			label: "Y",
			type: "number",
			usesVariables: true,
			numeric: {
				kind: "integer",
				signed: true,
				minimum: "-2147483648",
				maximum: "2147483647",
				minimumInclusive: true,
				maximumInclusive: true,
			},
		},
		{ key: "relative", label: "Relative move", type: "switch" },
	],
	defaultConfig: () => ({ x: "100", y: "100", relative: false }),
	description: "Move to a signed virtual-desktop coordinate or by a signed relative offset.",
	desktopOnly: true,
	fallible: true,
	group: "actions",
	icon: MousePointer2,
	kind: "action",
	label: "Move Mouse",
	permission: { name: "mouse_control", risk: "high" },
	risk: "high",
	runnerType: "move_mouse",
	supportedTargetRuntimes: ["Windows Desktop"],
	simulation: {
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Move Mouse (${node.id}) succeeded. Would move mouse ${api.getConfigString(node, "relative") === "true" ? "relatively by" : "to"} x=${api.formatValue(api.resolveTemplate(api.getConfigString(node, "x"), context))}, y=${api.formatValue(api.resolveTemplate(api.getConfigString(node, "y"), context))}.`,
			},
		],
	},
});

import { MousePointer } from "lucide-react";
import { defineNode } from "../../node-definition";
import { inputActionOptions, mouseButtonOptions, mouseClickTypeOptions } from "../options";
import { actionMouse } from "../shared";
import { configOption } from "../validators";

const inputActions = inputActionOptions.map((option) => option.value);
const mouseButtons = mouseButtonOptions.map((option) => option.value);
const mouseClickTypes = mouseClickTypeOptions.map((option) => option.value);

export const mouseClickNode = defineNode({
	actionType: "action.mouse",
	capabilities: actionMouse,
	configFields: [
		{
			key: "inputAction",
			label: "Input action",
			type: "select",
			options: inputActionOptions,
			required: false,
			help: "Press and release performs a normal click. Press down holds the button until a matching Release action or the run ends.",
		},
		{ key: "button", label: "Button", type: "select", options: mouseButtonOptions },
		{
			key: "clickType",
			label: "Click",
			type: "select",
			options: mouseClickTypeOptions,
			visibleWhen: { key: "inputAction", equals: "press" },
		},
	],
	defaultConfig: () => ({ inputAction: "press", button: "left", clickType: "single" }),
	description: "Click, hold, or release a mouse button.",
	desktopOnly: true,
	fallible: true,
	group: "actions",
	icon: MousePointer,
	kind: "action",
	label: "Mouse Click",
	permission: { name: "mouse_control", risk: "high" },
	risk: "high",
	runnerType: "mouse_click",
	supportedTargetRuntimes: ["Windows Desktop"],
	validateConfig: (config) => {
		const inputAction = typeof config.inputAction === "string" ? config.inputAction : "press";
		return [
			config.inputAction ? configOption(config, "inputAction", "input action", inputActions) : "",
			configOption(config, "button", "mouse button", mouseButtons),
			inputAction === "press" ? configOption(config, "clickType", "mouse click type", mouseClickTypes) : "",
		].filter(Boolean);
	},
	simulation: {
		describe: ({ api, node }) => {
			const inputAction = api.getConfigString(node, "inputAction") || "press";
			const button = api.getConfigString(node, "button");
			const actionLabel =
				inputAction === "down"
					? "press and hold"
					: inputAction === "up"
						? "release"
						: `${api.getConfigString(node, "clickType")} click`;
			return [
				{
					level: "info",
					message: `[Simulation] Mouse Click (${node.id}) succeeded. Would ${actionLabel} the ${button} mouse button.`,
				},
			];
		},
	},
});

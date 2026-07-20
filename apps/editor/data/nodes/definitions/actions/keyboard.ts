import { Keyboard } from "lucide-react";
import { defineNode } from "../../node-definition";
import { validateWindowsKeyExpression } from "../../windows-key-contract";
import { inputActionOptions } from "../options";
import { actionKeyboard } from "../shared";
import { configOption, configString, requiredStaticConfig } from "../validators";

const inputActions = inputActionOptions.map((option) => option.value);

export const keyboardNode = defineNode({
	actionType: "action.keyboard",
	capabilities: actionKeyboard,
	configFields: [
		{
			key: "inputAction",
			label: "Input action",
			type: "select",
			options: inputActionOptions,
			required: false,
			help: "Press and release performs a normal key press. Press down holds the keys until a matching Release action or the run ends.",
		},
		{ key: "key", label: "Key", type: "text" },
	],
	defaultConfig: () => ({ inputAction: "press", key: "Enter" }),
	description: "Press, hold, or release a Windows key or key chord.",
	desktopOnly: true,
	fallible: true,
	group: "actions",
	icon: Keyboard,
	kind: "action",
	label: "Keyboard",
	permission: { name: "keyboard_control", risk: "high" },
	risk: "high",
	runnerType: "press_key",
	supportedTargetRuntimes: ["Windows Desktop"],
	validateConfig: (config) => {
		const actionError = config.inputAction ? configOption(config, "inputAction", "input action", inputActions) : "";
		const requiredError = requiredStaticConfig(config, "key", "keyboard key");
		if (actionError || requiredError) {
			return [actionError, requiredError].filter(Boolean);
		}

		const error = validateWindowsKeyExpression(configString(config, "key"));
		return error ? [error] : [];
	},
	simulation: {
		describe: ({ api, node }) => {
			const inputAction = api.getConfigString(node, "inputAction") || "press";
			const actionLabel =
				inputAction === "down" ? "press and hold" : inputAction === "up" ? "release" : "press and release";
			return [
				{
					level: "info",
					message: `[Simulation] Keyboard (${node.id}) succeeded. Would ${actionLabel} keys ${api.getConfigString(node, "key")}.`,
				},
			];
		},
	},
});

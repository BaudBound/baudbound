import { Keyboard } from "lucide-react";
import { defineNode } from "../../node-definition";
import { validateWindowsKeyExpression } from "../../windows-key-contract";
import { actionKeyboard } from "../shared";
import { configString, requiredStaticConfig } from "../validators";

export const keyboardNode = defineNode({
	actionType: "action.keyboard",
	capabilities: actionKeyboard,
	configFields: [{ key: "key", label: "Key", type: "text" }],
	defaultConfig: () => ({ key: "Enter" }),
	description: "Send a Windows key or key chord.",
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
		const requiredError = requiredStaticConfig(config, "key", "keyboard key");
		if (requiredError) {
			return [requiredError];
		}

		const error = validateWindowsKeyExpression(configString(config, "key"));
		return error ? [error] : [];
	},
	simulation: {
		describe: ({ api, node }) => [
			{
				level: "info",
				message: `[Simulation] Keyboard (${node.id}) succeeded. Would press keys ${api.getConfigString(node, "key")}.`,
			},
		],
	},
});

import { Keyboard } from "lucide-react";
import { defineNode } from "../../node-definition";
import { actionKeyboard } from "../shared";
import { requiredConfig } from "../validators";

export const keyboardNode = defineNode({
	actionType: "action.keyboard",
	capabilities: actionKeyboard,
	configFields: [{ key: "key", label: "Key", type: "text" }],
	defaultConfig: () => ({ key: "Enter" }),
	description: "Send keyboard input.",
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
	validateConfig: (config) => [requiredConfig(config, "key", "keyboard key")].filter(Boolean),
	simulation: {
		describe: ({ api, node }) => [
			{
				level: "info",
				message: `[Simulation] Keyboard (${node.id}) succeeded. Would press keys ${api.getConfigString(node, "key")}.`,
			},
		],
	},
});

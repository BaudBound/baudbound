import { Keyboard } from "lucide-react";
import { defineNode } from "../../node-definition";
import { triggerPorts } from "../shared";

export const hotkeyTriggerNode = defineNode({
	actionType: "trigger.hotkey",
	capabilities: ["trigger.hotkey"],
	configFields: [{ key: "key", label: "Key", type: "text" }],
	defaultConfig: () => ({ key: "Ctrl+Alt+B" }),
	description: "Start from a desktop hotkey.",
	desktopOnly: true,
	group: "triggers",
	icon: Keyboard,
	kind: "trigger",
	label: "Hotkey",
	ports: triggerPorts,
	risk: "medium",
	runtimeOutputs: [
		{
			name: "key",
			type: "keyboard_key",
			description: "Captured hotkey expression.",
			example: "n-mr3zyt6f-4.key",
		},
	],
	runnerType: "hotkey",
	simulation: {
		createOutput: ({ api, context, node }) => ({
			failed: false,
			outputData: { key: context.triggerPayload.key || api.getConfigString(node, "key") },
		}),
	},
});

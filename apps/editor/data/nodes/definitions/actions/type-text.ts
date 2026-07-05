import { TextCursorInput } from "lucide-react";
import { defineNode } from "../../node-definition";
import { actionKeyboard } from "../shared";
import { requiredConfig } from "../validators";

export const typeTextNode = defineNode({
	actionType: "action.keyboard.type_text",
	capabilities: actionKeyboard,
	configFields: [{ key: "text", label: "Text", type: "textarea", usesVariables: true }],
	defaultConfig: () => ({ text: "Hello from BaudBound" }),
	description: "Type selected text through the keyboard.",
	desktopOnly: true,
	fallible: true,
	group: "actions",
	icon: TextCursorInput,
	kind: "action",
	label: "Type Text",
	permission: { name: "keyboard_control", risk: "high" },
	risk: "high",
	runnerType: "type_text",
	validateConfig: (config) => [requiredConfig(config, "text", "text to type")].filter(Boolean),
	simulation: {
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Type Text (${node.id}) succeeded. Would type text ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "text"), context))}.`,
			},
		],
	},
});

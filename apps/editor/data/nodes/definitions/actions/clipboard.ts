import { Clipboard } from "lucide-react";
import { defineNode } from "../../node-definition";

export const clipboardNode = defineNode({
	actionType: "action.clipboard",
	capabilities: ["action.clipboard"],
	configFields: [{ key: "value", label: "Value", type: "textarea", usesVariables: true }],
	defaultConfig: () => ({ value: "Copied text: {{status}}" }),
	description: "Read or write clipboard data.",
	desktopOnly: true,
	fallible: true,
	group: "actions",
	icon: Clipboard,
	kind: "action",
	label: "Clipboard",
	permission: { name: "read_clipboard", risk: "high" },
	risk: "high",
	runnerType: "set_clipboard",
	simulation: {
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Clipboard (${node.id}) succeeded. Would write clipboard value ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "value"), context))}.`,
			},
		],
	},
});

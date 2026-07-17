import { Clipboard } from "lucide-react";
import { defineNode } from "../../node-definition";
import { fallible } from "../runtime-outputs";

export const clipboardNode = defineNode({
	actionType: "action.clipboard.set",
	capabilities: ["action.clipboard"],
	configFields: [{ key: "value", label: "Value", type: "textarea", usesVariables: true }],
	defaultConfig: () => ({ value: "Clipboard text" }),
	description: "Write text to the system clipboard.",
	desktopOnly: true,
	fallible: true,
	group: "actions",
	icon: Clipboard,
	kind: "action",
	label: "Set Clipboard",
	permission: { name: "write_clipboard", risk: "medium" },
	risk: "medium",
	runtimeOutputs: fallible([
		{ name: "value", type: "string", description: "Text written to the clipboard.", example: "n-mr3zyt6f-20.value" },
		{
			name: "bytes",
			type: "number",
			description: "UTF-8 byte length of the written text.",
			example: "n-mr3zyt6f-20.bytes",
		},
	]),
	runnerType: "set_clipboard",
	simulation: {
		createOutput: ({ api, context, node }) => {
			const value = String(api.resolveTemplate(api.getConfigString(node, "value"), context));
			return {
				failed: false,
				outputData: { bytes: new TextEncoder().encode(value).length, value },
			};
		},
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Set Clipboard (${node.id}) succeeded. Would write clipboard value ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "value"), context))}.`,
			},
		],
	},
});

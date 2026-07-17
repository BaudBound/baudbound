import { ClipboardPaste } from "lucide-react";
import { defineNode } from "../../node-definition";
import { fallible } from "../runtime-outputs";

export const getClipboardNode = defineNode({
	actionType: "action.clipboard.get",
	capabilities: ["action.clipboard"],
	configFields: [],
	defaultConfig: () => ({}),
	description: "Read text from the system clipboard.",
	desktopOnly: true,
	fallible: true,
	group: "actions",
	icon: ClipboardPaste,
	kind: "action",
	label: "Get Clipboard",
	permission: { name: "read_clipboard", risk: "medium" },
	risk: "medium",
	runtimeOutputs: fallible([
		{ name: "text", type: "string", description: "Text read from the clipboard.", example: "n-mr3zyt6f-21.text" },
	]),
	runnerType: "get_clipboard",
	simulation: {
		createOutput: () => ({
			failed: false,
			outputData: { text: "Simulated clipboard text" },
		}),
		describe: ({ context, node }) => [
			{
				level: "info",
				message: `[Simulation] Get Clipboard (${node.id}) succeeded. Read ${JSON.stringify(String(context.nodeOutputs[node.id]?.text ?? ""))}.`,
			},
		],
	},
});

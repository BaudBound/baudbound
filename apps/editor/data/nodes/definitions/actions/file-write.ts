import { FileText } from "lucide-react";
import { defineNode } from "../../node-definition";
import { fileWriteModeOptions } from "../options";
import { actionFile } from "../shared";

export const writeFileNode = defineNode({
	actionType: "action.file.write",
	capabilities: actionFile,
	configFields: [
		{ key: "mode", label: "Write mode", type: "select", options: fileWriteModeOptions },
		{ key: "path", label: "Path", type: "text", usesVariables: true },
		{ key: "content", label: "Content", type: "textarea", usesVariables: true },
	],
	defaultConfig: () => ({ mode: "overwrite", path: "./output.txt", content: "Hello from BaudBound" }),
	description: "Write to a file path.",
	fallible: true,
	group: "actions",
	icon: FileText,
	kind: "action",
	label: "Write File",
	permission: { name: "file_write_limited", risk: "high" },
	risk: "high",
	runnerType: "write_file",
	simulation: {
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Write File (${node.id}) succeeded. Would ${api.getConfigString(node, "mode") === "append" ? "append to" : "overwrite"} ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "path"), context))}.`,
			},
		],
	},
});

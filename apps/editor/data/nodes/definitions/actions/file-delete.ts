import { Trash2 } from "lucide-react";
import { defineNode } from "../../node-definition";
import { fallible } from "../runtime-outputs";
import { actionFile } from "../shared";
import { requiredConfig } from "../validators";

export const deleteFileNode = defineNode({
	actionType: "action.file.delete",
	capabilities: actionFile,
	configFields: [{ key: "path", label: "Path", type: "text", usesVariables: true }],
	defaultConfig: () => ({ path: "./old-file.txt" }),
	description: "Delete a selected file path.",
	fallible: true,
	group: "actions",
	icon: Trash2,
	kind: "action",
	label: "Delete File",
	permission: { name: "delete_file", risk: "dangerous" },
	risk: "dangerous",
	runtimeOutputs: fallible([
		{ name: "path", type: "file_path", description: "Deleted file path.", example: "n-mr3zyt6f-20.path" },
	]),
	runnerType: "delete_file",
	validateConfig: (config) => [requiredConfig(config, "path", "file path")].filter(Boolean),
	simulation: {
		createOutput: ({ api, context, node }) => ({
			failed: false,
			outputData: { path: String(api.resolveTemplate(api.getConfigString(node, "path"), context)) },
		}),
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Delete File (${node.id}) succeeded. Would delete ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "path"), context))}.`,
			},
		],
	},
});

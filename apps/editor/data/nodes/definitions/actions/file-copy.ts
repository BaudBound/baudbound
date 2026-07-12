import { Copy } from "lucide-react";
import { defineNode } from "../../node-definition";
import { fileOverwriteOptions } from "../options";
import { fallible, fileTransferRuntimeOutputs } from "../runtime-outputs";
import { actionFile } from "../shared";
import { requiredConfig } from "../validators";

export const copyFileNode = defineNode({
	actionType: "action.file.copy",
	capabilities: actionFile,
	configFields: [
		{ key: "sourcePath", label: "Source path", type: "text", usesVariables: true },
		{ key: "destinationPath", label: "Destination path", type: "text", usesVariables: true },
		{ key: "overwrite", label: "Overwrite", type: "select", options: fileOverwriteOptions },
	],
	defaultConfig: () => ({ sourcePath: "./input.txt", destinationPath: "./backup/input.txt", overwrite: "false" }),
	description: "Copy a file to another location.",
	fallible: true,
	group: "actions",
	icon: Copy,
	kind: "action",
	label: "Copy File",
	permission: { name: "file_copy", risk: "medium" },
	permissionPathRules: [
		{ access: "read", configKey: "sourcePath" },
		{ access: "write", configKey: "destinationPath" },
	],
	risk: "medium",
	runtimeOutputs: fallible(fileTransferRuntimeOutputs("copied")),
	runnerType: "copy_file",
	validateConfig: (config) =>
		[
			requiredConfig(config, "sourcePath", "source file path"),
			requiredConfig(config, "destinationPath", "destination file path"),
		].filter(Boolean),
	simulation: {
		createOutput: ({ api, context, node }) => ({
			failed: false,
			outputData: {
				source_path: String(api.resolveTemplate(api.getConfigString(node, "sourcePath"), context)),
				destination_path: String(api.resolveTemplate(api.getConfigString(node, "destinationPath"), context)),
			},
		}),
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Copy File (${node.id}) succeeded. Would copy ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "sourcePath"), context))} to ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "destinationPath"), context))}${api.getConfigString(node, "overwrite") === "true" ? " and overwrite an existing destination" : ""}.`,
			},
		],
	},
});

import { Download } from "lucide-react";
import { createWriteFilePermission, fileWriteLimitedPermission } from "@/data/project/file-permissions";
import { defineNode } from "../../node-definition";
import { fileOverwriteOptions } from "../options";
import { fallible } from "../runtime-outputs";
import { actionFile } from "../shared";
import { requiredConfig, staticHttpUrlConfig } from "../validators";

export const downloadFileNode = defineNode({
	actionType: "action.file.download",
	capabilities: actionFile,
	configFields: [
		{ key: "url", label: "URL", type: "text", usesVariables: true },
		{ key: "destinationPath", label: "Destination path", type: "text", usesVariables: true },
		{ key: "overwrite", label: "Overwrite", type: "select", options: fileOverwriteOptions },
	],
	defaultConfig: () => ({
		url: "https://example.com/file.txt",
		destinationPath: "./downloads/file.txt",
		overwrite: "false",
	}),
	description: "Download a URL to a file path.",
	fallible: true,
	group: "actions",
	icon: Download,
	kind: "action",
	label: "Download File",
	permission: { name: "download_file", risk: "medium" },
	derivePermissions: (config) => {
		const writePermission = createWriteFilePermission(config.destinationPath);
		return [
			{ name: "download_file", risk: "medium" },
			...(writePermission.name === fileWriteLimitedPermission.name ? [] : [writePermission]),
		];
	},
	risk: "medium",
	runtimeOutputs: fallible([
		{
			name: "path",
			type: "file_path",
			description: "Destination file path written by the runner.",
			example: "n-mr3zyt6f-19.path",
		},
		{ name: "url", type: "string", description: "Source URL used for the download.", example: "n-mr3zyt6f-19.url" },
	]),
	runnerType: "download_file",
	validateConfig: (config) =>
		[
			requiredConfig(config, "url", "download URL"),
			staticHttpUrlConfig(config, "url", "download URL"),
			requiredConfig(config, "destinationPath", "destination file path"),
		].filter(Boolean),
	simulation: {
		createOutput: ({ api, context, node }) => ({
			failed: false,
			outputData: {
				url: String(api.resolveTemplate(api.getConfigString(node, "url"), context)),
				path: String(api.resolveTemplate(api.getConfigString(node, "destinationPath"), context)),
			},
		}),
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Download File (${node.id}) succeeded. Would download ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "url"), context))} to ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "destinationPath"), context))}${getOverwriteDetail(api.getConfigString(node, "overwrite"))}.`,
			},
		],
	},
});

function getOverwriteDetail(value: string) {
	return value === "true" ? " and overwrite an existing destination" : "";
}

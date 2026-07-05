import { FileInput } from "lucide-react";
import { createReadFilePermission } from "@/data/project/file-permissions";
import { defineNode } from "../../node-definition";
import { fallible } from "../runtime-outputs";
import { actionFile } from "../shared";
import { requiredConfig } from "../validators";

export const readFileNode = defineNode({
	actionType: "action.file.read",
	capabilities: actionFile,
	configFields: [
		{ key: "path", label: "Path", type: "text", usesVariables: true },
		{ key: "encoding", label: "Encoding", type: "select", options: [{ value: "utf-8", label: "UTF-8" }] },
	],
	defaultConfig: () => ({ path: "./input.txt", encoding: "utf-8" }),
	description: "Read file content into runtime data.",
	fallible: true,
	group: "actions",
	icon: FileInput,
	kind: "action",
	label: "Read File",
	permission: { name: "file_read", risk: "medium" },
	derivePermissions: (config) => [createReadFilePermission(config.path)],
	risk: "medium",
	runtimeOutputs: fallible([
		{ name: "path", type: "file_path", description: "File path read by the runner.", example: "n-mr3zyt6f-19.path" },
		{
			name: "content",
			type: "file_content",
			description: "Text content read from the file.",
			example: "n-mr3zyt6f-19.content",
		},
		{ name: "bytes", type: "number", description: "Number of bytes read.", example: "n-mr3zyt6f-19.bytes" },
	]),
	runnerType: "read_file",
	validateConfig: (config) => [requiredConfig(config, "path", "file path")].filter(Boolean),
	simulation: {
		createOutput: ({ api, context, node }) => {
			const path = String(api.resolveTemplate(api.getConfigString(node, "path"), context));
			const content = `[Simulation] File content from ${path}`;
			return {
				failed: false,
				outputData: {
					path,
					content,
					bytes: new TextEncoder().encode(content).length,
				},
			};
		},
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Read File (${node.id}) succeeded. Would read ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "path"), context))} as ${api.getConfigString(node, "encoding") || "utf-8"}.`,
			},
		],
	},
});

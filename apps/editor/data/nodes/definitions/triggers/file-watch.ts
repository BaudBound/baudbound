import { FileText } from "lucide-react";
import { defineNode } from "../../node-definition";
import { triggerPorts } from "../shared";
import { requiredStaticConfig } from "../validators";

export const fileWatchTriggerNode = defineNode({
	actionType: "trigger.file_watch",
	capabilities: ["trigger.file_watch"],
	configFields: [
		{ key: "path", label: "Path", type: "text" },
		{
			key: "recursive",
			label: "Include subdirectories",
			type: "switch",
			required: false,
			help: "Applies when Path is a directory.",
		},
	],
	defaultConfig: () => ({ path: "/path/to/watch", recursive: false }),
	description: "Start when a file changes.",
	group: "triggers",
	icon: FileText,
	kind: "trigger",
	label: "File Watch",
	ports: triggerPorts,
	risk: "low",
	runtimeOutputs: [
		{
			name: "path",
			type: "file_path",
			description: "Changed file path.",
			example: "n-mr3zyt6f-2.path",
		},
		{
			name: "event",
			type: "string",
			description: "File event type reported by the runner.",
			example: "n-mr3zyt6f-2.event",
		},
	],
	runnerType: "file_watch",
	validateConfig: (config) => [requiredStaticConfig(config, "path", "file watch path")].filter(Boolean),
	simulation: {
		createOutput: ({ api, context, node }) => ({
			failed: false,
			outputData: {
				path: context.triggerPayload.path || api.resolveTemplate(api.getConfigString(node, "path"), context),
				event: context.triggerPayload.event || "modified",
			},
		}),
		describe: ({ api, context, node }) => {
			const output = context.nodeOutputs[node.id];
			const event = typeof output?.event === "string" ? output.event : "modified";
			const path =
				typeof output?.path === "string"
					? output.path
					: api.formatValue(api.resolveTemplate(api.getConfigString(node, "path"), context));

			return [{ level: "info", message: `[Simulation] File watcher received ${event} event for ${path}.` }];
		},
	},
});

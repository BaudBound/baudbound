import { AppWindow } from "lucide-react";
import { defineNode } from "../../node-definition";
import { fallible } from "../runtime-outputs";
import { actionWindow } from "../shared";

export const getActiveWindowNode = defineNode({
	actionType: "action.window.active",
	capabilities: actionWindow,
	configFields: [],
	defaultConfig: () => ({}),
	description: "Read the active foreground window.",
	desktopOnly: true,
	fallible: true,
	group: "actions",
	icon: AppWindow,
	kind: "action",
	label: "Get Active Window",
	permission: { name: "window_query", risk: "medium" },
	risk: "medium",
	supportedTargetRuntimes: ["Windows Desktop"],
	runtimeOutputs: fallible([
		{ name: "title", type: "string", description: "Active window title.", example: "n-mr3zyt6f-17.title" },
		{
			name: "process_name",
			type: "string",
			description: "Process that owns the active window.",
			example: "n-mr3zyt6f-17.process_name",
		},
		{
			name: "process_id",
			type: "process_id",
			description: "Owning process identifier.",
			example: "n-mr3zyt6f-17.process_id",
		},
		{
			name: "executable_path",
			type: "file_path",
			description: "Owning process executable path when available.",
			example: "n-mr3zyt6f-17.executable_path",
		},
	]),
	runnerType: "get_active_window",
	simulation: {
		createOutput: () => ({
			failed: false,
			outputData: {
				title: "Simulated Active Window",
				process_name: "app.exe",
				process_id: 4245,
				executable_path: "C:\\Program Files\\App\\app.exe",
			},
		}),
		describe: ({ context, node }) => [
			{
				level: "info",
				message: `[Simulation] Get Active Window (${node.id}) succeeded. Captured active window "${String(context.nodeOutputs[node.id]?.title ?? "unknown")}".`,
			},
		],
	},
});

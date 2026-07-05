import { AppWindow } from "lucide-react";
import { defineNode } from "../../node-definition";
import { fallible } from "../runtime-outputs";
import { actionWindow } from "../shared";
import { requiredConfig } from "../validators";

export const openApplicationNode = defineNode({
	actionType: "action.application.open",
	capabilities: actionWindow,
	configFields: [
		{
			key: "application",
			label: "Application",
			type: "text",
			usesVariables: true,
			help: "Use an app name, app id, bundle id, shortcut path, or desktop entry supported by the target runner.",
		},
		{ key: "arguments", label: "Arguments", type: "textarea", usesVariables: true },
	],
	defaultConfig: () => ({ application: "Calculator", arguments: "" }),
	description: "Open an installed desktop application.",
	desktopOnly: true,
	fallible: true,
	group: "actions",
	icon: AppWindow,
	kind: "action",
	label: "Open Application",
	permission: { name: "open_application", risk: "medium" },
	risk: "medium",
	runtimeOutputs: fallible([
		{
			name: "application_id",
			type: "string",
			description: "Resolved application identifier used by the runner.",
			example: "n-mr3zyt6f-16.application_id",
		},
		{
			name: "process_id",
			type: "process_id",
			description: "Started process identifier when the platform exposes one.",
			example: "n-mr3zyt6f-16.process_id",
		},
	]),
	runnerType: "open_application",
	validateConfig: (config) => [requiredConfig(config, "application", "application")].filter(Boolean),
	simulation: {
		createOutput: ({ api, node }) => ({
			failed: false,
			outputData: { application_id: api.getConfigString(node, "application") || "application", process_id: 4243 },
		}),
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Open Application (${node.id}) succeeded. Would open application ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "application"), context))}.`,
			},
		],
	},
});

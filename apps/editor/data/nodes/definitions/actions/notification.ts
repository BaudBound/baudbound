import { Bell } from "lucide-react";
import { defineNode } from "../../node-definition";
import { requiredConfig } from "../validators";

export const notificationNode = defineNode({
	actionType: "action.notification",
	capabilities: ["action.notification"],
	configFields: [
		{ key: "title", label: "Title", type: "text", usesVariables: true },
		{ key: "message", label: "Message", type: "textarea", usesVariables: true },
	],
	defaultConfig: () => ({ title: "BaudBound", message: "Notification message" }),
	description: "Show a desktop notification.",
	desktopOnly: true,
	fallible: true,
	group: "actions",
	icon: Bell,
	kind: "action",
	label: "Show Notification",
	permission: { name: "show_notification", risk: "medium" },
	risk: "medium",
	runnerType: "show_notification",
	validateConfig: (config) =>
		[
			requiredConfig(config, "title", "notification title"),
			requiredConfig(config, "message", "notification message"),
		].filter(Boolean),
	simulation: {
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Show Notification (${node.id}) succeeded. Would show notification "${api.resolveTemplate(api.getConfigString(node, "title"), context)}" with message "${api.resolveTemplate(api.getConfigString(node, "message"), context)}".`,
			},
		],
		sideEffects: ({ api, context, node }) => [
			{
				type: "notification_toast",
				nodeId: node.id,
				title: String(api.resolveTemplate(api.getConfigString(node, "title"), context)),
				message: String(api.resolveTemplate(api.getConfigString(node, "message"), context)),
			},
		],
	},
});

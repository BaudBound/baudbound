import { MessageSquareWarning } from "lucide-react";
import type { SimulationSideEffect } from "@/utils/simulation-types";
import { defineNode } from "../../node-definition";
import { messageBoxButtonOptions, messageBoxTypeOptions } from "../options";

export const messageBoxNode = defineNode({
	actionType: "action.message_box",
	capabilities: ["action.message_box"],
	configFields: [
		{ key: "type", label: "Type", type: "select", options: messageBoxTypeOptions },
		{ key: "buttons", label: "Buttons", type: "select", options: messageBoxButtonOptions },
		{ key: "title", label: "Title", type: "text", usesVariables: true },
		{ key: "message", label: "Message", type: "textarea", usesVariables: true },
	],
	defaultConfig: () => ({ type: "info", buttons: "ok", title: "BaudBound", message: "Script says hello." }),
	description: "Show an operating-system message box.",
	desktopOnly: true,
	fallible: true,
	group: "actions",
	icon: MessageSquareWarning,
	kind: "action",
	label: "MessageBox",
	permission: { name: "show_message_box", risk: "medium" },
	risk: "medium",
	runtimeOutputs: [
		{
			name: "button",
			type: "string",
			description: "Button selected by the user.",
			example: "n-mr3zyt6f-18.button",
		},
	],
	runnerType: "show_message_box",
	simulation: {
		createOutput: () => ({ failed: false, outputData: {} }),
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] MessageBox (${node.id}) succeeded. Would show ${api.getConfigString(node, "type")} message box "${api.resolveTemplate(api.getConfigString(node, "title"), context)}".`,
			},
		],
		sideEffects: ({ api, context, node }) => [
			{
				type: "message_box",
				nodeId: node.id,
				title: String(api.resolveTemplate(api.getConfigString(node, "title"), context)),
				message: String(api.resolveTemplate(api.getConfigString(node, "message"), context)),
				variant: normalizeMessageBoxVariant(api.getConfigString(node, "type")),
				buttons: getMessageBoxButtons(api.getConfigString(node, "buttons")),
			},
		],
	},
});

function getMessageBoxButtons(value: string) {
	switch (value) {
		case "ok_cancel":
			return ["ok", "cancel"];
		case "yes_no":
			return ["yes", "no"];
		case "yes_no_cancel":
			return ["yes", "no", "cancel"];
		case "retry_cancel":
			return ["retry", "cancel"];
		default:
			return ["ok"];
	}
}

function normalizeMessageBoxVariant(value: string): Extract<SimulationSideEffect, { type: "message_box" }>["variant"] {
	if (value === "warning" || value === "error" || value === "question") {
		return value;
	}

	return "info";
}

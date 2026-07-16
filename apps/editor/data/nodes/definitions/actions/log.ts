import { Terminal } from "lucide-react";
import { defineNode } from "../../node-definition";
import { logLevelOptions } from "../options";
import { requiredConfig } from "../validators";

export const logNode = defineNode({
	actionType: "action.log",
	capabilities: ["action.log"],
	configFields: [
		{ key: "level", label: "Log level", type: "select", options: logLevelOptions },
		{ key: "message", label: "Message", type: "textarea", usesVariables: true },
	],
	defaultConfig: () => ({
		level: "info",
		message: "Log message",
	}),
	description: "Write a runner log message.",
	group: "actions",
	icon: Terminal,
	kind: "action",
	label: "Log",
	permission: { name: "log", risk: "low" },
	risk: "low",
	runnerType: "log",
	validateConfig: (config) => [requiredConfig(config, "message", "log message")].filter(Boolean),
	simulation: {
		outputLogs: ({ api, context, failed, node }) =>
			failed
				? []
				: [
						{
							level: normalizeLogLevel(api.getConfigString(node, "level")),
							message: String(api.resolveTemplate(api.getConfigString(node, "message"), context)),
						},
					],
	},
});

function normalizeLogLevel(value: string): "info" | "warn" | "error" | "debug" {
	if (value === "debug" || value === "warn" || value === "error") {
		return value;
	}

	return "info";
}

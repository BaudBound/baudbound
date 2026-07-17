import { Bell } from "lucide-react";
import type { JsonValue } from "@/lib/types";
import { defineNode } from "../../node-definition";
import { actionAudio } from "../shared";
import { configString, hasTemplateReference, staticPositiveNumberConfig } from "../validators";

export const beepNode = defineNode({
	actionType: "action.beep",
	capabilities: actionAudio,
	configFields: [
		{
			key: "frequencyHz",
			label: "Frequency Hz",
			type: "number",
			usesVariables: true,
			numeric: {
				kind: "float",
				signed: false,
				minimum: "20",
				maximum: "20000",
				minimumInclusive: true,
				maximumInclusive: true,
			},
		},
		{
			key: "durationMs",
			label: "Duration ms",
			type: "number",
			usesVariables: true,
			numeric: {
				kind: "float",
				signed: false,
				minimum: "10",
				maximum: "5000",
				minimumInclusive: true,
				maximumInclusive: true,
			},
		},
	],
	defaultConfig: () => ({ frequencyHz: "800", durationMs: "200" }),
	description: "Play a tone through the default audio output.",
	desktopOnly: true,
	fallible: true,
	group: "actions",
	icon: Bell,
	kind: "action",
	label: "Beep",
	permission: { name: "beep", risk: "low" },
	risk: "low",
	runnerType: "beep",
	validateConfig: (config) =>
		[
			boundedBeepConfig(config, "frequencyHz", "beep frequency", 20, 20_000),
			boundedBeepConfig(config, "durationMs", "beep duration", 10, 5_000),
		].filter(Boolean),
	simulation: {
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Beep (${node.id}) succeeded. Played simulated beep at ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "frequencyHz"), context))}Hz for ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "durationMs"), context))}ms.`,
			},
		],
		sideEffects: ({ api, context, node }) => [
			{
				type: "system_beep",
				nodeId: node.id,
				frequencyHz: Number(api.resolveTemplate(api.getConfigString(node, "frequencyHz"), context)),
				durationMs: Number(api.resolveTemplate(api.getConfigString(node, "durationMs"), context)),
			},
		],
	},
});

function boundedBeepConfig(
	config: Record<string, JsonValue>,
	key: string,
	label: string,
	minimum: number,
	maximum: number,
) {
	const positiveError = staticPositiveNumberConfig(config, key, label);
	const rawValue = configString(config, key);
	if (positiveError || hasTemplateReference(rawValue)) {
		return positiveError;
	}
	const value = Number(rawValue);
	return value >= minimum && value <= maximum ? "" : `${label} must be between ${minimum} and ${maximum}.`;
}

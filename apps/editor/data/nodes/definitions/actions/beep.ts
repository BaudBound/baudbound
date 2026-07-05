import { Bell } from "lucide-react";
import { defineNode } from "../../node-definition";
import { actionAudio } from "../shared";

export const beepNode = defineNode({
	actionType: "action.beep",
	capabilities: actionAudio,
	configFields: [
		{ key: "frequencyHz", label: "Frequency Hz", type: "number", usesVariables: true },
		{ key: "durationMs", label: "Duration ms", type: "number", usesVariables: true },
	],
	defaultConfig: () => ({ frequencyHz: "800", durationMs: "200" }),
	description: "Play the system beeper.",
	fallible: true,
	group: "actions",
	icon: Bell,
	kind: "action",
	label: "Beep",
	permission: { name: "system_beep", risk: "low" },
	risk: "low",
	runnerType: "beep",
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
				frequencyHz: api.clampNumber(
					Number(api.resolveTemplate(api.getConfigString(node, "frequencyHz"), context)) || 800,
					20,
					20000,
				),
				durationMs: api.clampNumber(
					Number(api.resolveTemplate(api.getConfigString(node, "durationMs"), context)) || 200,
					10,
					5000,
				),
			},
		],
	},
});

import type { Node } from "@xyflow/react";
import { Volume2 } from "lucide-react";
import type { ScriptNodeData } from "@/lib/types";
import type { SimulationContext } from "@/utils/simulation-types";
import type { NodeSimulationApi } from "../../node-definition";
import { defineNode } from "../../node-definition";
import { actionAudio } from "../shared";

export const playSoundNode = defineNode({
	actionType: "action.sound.play",
	capabilities: actionAudio,
	configFields: [],
	defaultConfig: () => ({ source: "asset", assetPath: "", filePath: "" }),
	description: "Play an audio file from assets or a file path.",
	fallible: true,
	group: "actions",
	icon: Volume2,
	kind: "action",
	label: "Play Sound",
	permission: { name: "play_audio", risk: "medium" },
	risk: "medium",
	runnerType: "play_sound",
	simulation: {
		createOutput: ({ api, context, node }) => api.validatePlaySound(node, context),
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Play Sound (${node.id}) succeeded. ${getPlaySoundExecutionDetail(api, node, context)}`,
			},
		],
		sideEffects: ({ api, context, node }) => {
			if (api.getConfigString(node, "source") === "file_path") {
				return [];
			}

			const assetPath = String(api.resolveTemplate(api.getConfigString(node, "assetPath"), context)).trim();
			return assetPath ? [{ type: "play_audio_asset", nodeId: node.id, assetPath }] : [];
		},
	},
});

function getPlaySoundExecutionDetail(api: NodeSimulationApi, node: Node<ScriptNodeData>, context: SimulationContext) {
	const source = api.getConfigString(node, "source") === "file_path" ? "file_path" : "asset";
	if (source === "file_path") {
		return `Would play audio file ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "filePath"), context))}.`;
	}

	return `Would play packaged audio asset ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "assetPath"), context))}.`;
}

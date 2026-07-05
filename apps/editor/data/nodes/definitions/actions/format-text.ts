import { TextCursorInput } from "lucide-react";
import { defineNode } from "../../node-definition";
import { requiredConfig } from "../validators";

export const formatTextNode = defineNode({
	actionType: "action.text.format",
	capabilities: ["action.text"],
	configFields: [{ key: "template", label: "Template", type: "textarea", usesVariables: true }],
	defaultConfig: () => ({ template: "Hello {{item}}" }),
	description: "Format template text and expose the result.",
	group: "actions",
	icon: TextCursorInput,
	kind: "action",
	label: "Format Text",
	permission: { name: "text_transform", risk: "low" },
	risk: "low",
	runtimeOutputs: [
		{
			name: "text",
			type: "string",
			description: "Formatted text after variable/runtime references are resolved.",
			example: "n-mr3zyt6f-18.text",
		},
	],
	runnerType: "format_text",
	validateConfig: (config) => [requiredConfig(config, "template", "format template")].filter(Boolean),
	simulation: {
		createOutput: ({ api, context, node }) => ({
			failed: false,
			outputData: { text: String(api.resolveTemplate(api.getConfigString(node, "template"), context)) },
		}),
		describe: ({ context, node }) => [
			{
				level: "info",
				message: `[Simulation] Formatted text as "${String(context.nodeOutputs[node.id]?.text ?? "")}".`,
			},
		],
	},
});

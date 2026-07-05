import { Pipette } from "lucide-react";
import { defineNode } from "../../node-definition";
import { fallible } from "../runtime-outputs";

export const getPixelColorNode = defineNode({
	actionType: "action.pixel.get",
	capabilities: ["action.screen"],
	configFields: [
		{ key: "x", label: "Screen X", type: "number", usesVariables: true },
		{ key: "y", label: "Screen Y", type: "number", usesVariables: true },
	],
	defaultConfig: () => ({ x: "100", y: "100" }),
	description: "Read the screen pixel color at an X/Y coordinate.",
	desktopOnly: true,
	fallible: true,
	group: "actions",
	icon: Pipette,
	kind: "action",
	label: "Get Pixel Color",
	permission: { name: "screen_pixel_read", risk: "high" },
	risk: "high",
	runtimeOutputs: fallible([
		{ name: "hex", type: "string", description: "Pixel color as a hex string.", example: "n-mr3zyt6f-19.hex" },
		{
			name: "rgb",
			type: "object",
			description: "Pixel color as red, green, and blue channels.",
			example: "n-mr3zyt6f-19.rgb.r",
			fields: [
				{ name: "r", type: "number", description: "Red channel, 0-255." },
				{ name: "g", type: "number", description: "Green channel, 0-255." },
				{ name: "b", type: "number", description: "Blue channel, 0-255." },
			],
		},
		{
			name: "rgba",
			type: "object",
			description: "Pixel color as red, green, blue, and alpha channels.",
			example: "n-mr3zyt6f-19.rgba.a",
			fields: [
				{ name: "r", type: "number", description: "Red channel, 0-255." },
				{ name: "g", type: "number", description: "Green channel, 0-255." },
				{ name: "b", type: "number", description: "Blue channel, 0-255." },
				{ name: "a", type: "number", description: "Alpha channel, 0-255." },
			],
		},
		{ name: "red", type: "number", description: "Red channel, 0-255.", example: "n-mr3zyt6f-19.red" },
		{ name: "green", type: "number", description: "Green channel, 0-255.", example: "n-mr3zyt6f-19.green" },
		{ name: "blue", type: "number", description: "Blue channel, 0-255.", example: "n-mr3zyt6f-19.blue" },
		{ name: "alpha", type: "number", description: "Alpha channel, 0-255.", example: "n-mr3zyt6f-19.alpha" },
		{ name: "integer", type: "number", description: "Packed RGB integer value.", example: "n-mr3zyt6f-19.integer" },
	]),
	runnerType: "get_pixel_color",
	simulation: {
		createOutput: ({ api, context, node }) => ({
			failed: false,
			outputData: api.createPixelColorOutput(
				Number(api.resolveTemplate(api.getConfigString(node, "x"), context)) || 0,
				Number(api.resolveTemplate(api.getConfigString(node, "y"), context)) || 0,
			),
		}),
		describe: ({ api, context, node }) => {
			const output = context.nodeOutputs[node.id];
			const x = api.formatValue(api.resolveTemplate(api.getConfigString(node, "x"), context));
			const y = api.formatValue(api.resolveTemplate(api.getConfigString(node, "y"), context));

			return [
				{
					level: "info",
					message: `[Simulation] Get Pixel Color (${node.id}) succeeded. Captured simulated screen pixel at x=${x}, y=${y} as ${String(output?.hex ?? "unknown")}.`,
				},
			];
		},
	},
});

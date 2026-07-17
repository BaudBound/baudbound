import { Blend } from "lucide-react";
import { evaluateColorMatch, validateStaticColor } from "../../color-match";
import { defineNode } from "../../node-definition";

export const colorMatchNode = defineNode({
	actionType: "control.color_match",
	capabilities: ["runtime.color_match"],
	configFields: [
		{
			colorPicker: true,
			key: "actualColor",
			label: "Actual color",
			type: "text",
			usesVariables: true,
			help: "Use #RRGGBB, rgb(r, g, b), or an RGB object variable such as a Get Pixel Color rgb output.",
		},
		{
			colorPicker: true,
			key: "expectedColor",
			label: "Expected color",
			type: "text",
			usesVariables: true,
			help: "The color to compare against the actual color.",
		},
		{
			key: "comparisonMode",
			label: "Comparison mode",
			type: "select",
			options: [
				{ label: "Per channel", value: "per_channel" },
				{ label: "Total RGB distance", value: "total_distance" },
			],
		},
		{
			key: "tolerancePercent",
			label: "Tolerance (%)",
			type: "number",
			usesVariables: true,
			numeric: {
				kind: "float",
				signed: false,
				minimum: "0",
				maximum: "100",
				minimumInclusive: true,
				maximumInclusive: true,
			},
			help: "Zero requires an exact match. One hundred accepts every valid RGB pair.",
		},
	],
	controlType: "color_match",
	defaultConfig: () => ({
		actualColor: "#000000",
		expectedColor: "#000000",
		comparisonMode: "per_channel",
		tolerancePercent: "0",
	}),
	description: "Branch by RGB similarity using percentage tolerance.",
	group: "control",
	icon: Blend,
	kind: "control",
	label: "Color Match",
	portPolicy: { kind: "fixed", inputs: ["input"], outputs: ["match", "no_match"] },
	risk: "low",
	runtimeOutputs: [
		{
			name: "matches",
			type: "boolean",
			description: "Whether the colors matched within the configured tolerance.",
			example: "n-mr3zyt6f-20.matches",
		},
		{
			name: "difference_percent",
			type: "number",
			description: "The normalized difference used by the selected comparison mode.",
			example: "n-mr3zyt6f-20.difference_percent",
		},
		{
			name: "red_difference",
			type: "number",
			description: "Absolute red channel difference from 0 through 255.",
			example: "n-mr3zyt6f-20.red_difference",
		},
		{
			name: "green_difference",
			type: "number",
			description: "Absolute green channel difference from 0 through 255.",
			example: "n-mr3zyt6f-20.green_difference",
		},
		{
			name: "blue_difference",
			type: "number",
			description: "Absolute blue channel difference from 0 through 255.",
			example: "n-mr3zyt6f-20.blue_difference",
		},
	],
	validateConfig: (config) => {
		const errors: string[] = [];
		for (const [key, label] of [
			["actualColor", "Actual color"],
			["expectedColor", "Expected color"],
		] as const) {
			const value = config[key];
			if (typeof value !== "string" || !value.trim()) {
				errors.push(`${label} is required.`);
				continue;
			}
			const error = validateStaticColor(value, label.toLowerCase());
			if (error) errors.push(`${error}.`);
		}
		return errors;
	},
	simulation: {
		createOutput: ({ api, context, node }) => {
			const actual = api.resolveTemplate(api.getConfigString(node, "actualColor"), context);
			const expected = api.resolveTemplate(api.getConfigString(node, "expectedColor"), context);
			const mode = api.getConfigString(node, "comparisonMode");
			const tolerance = Number(api.resolveTemplate(api.getConfigString(node, "tolerancePercent"), context));
			const evaluation = evaluateColorMatch(actual, expected, mode, tolerance);
			if (!evaluation.ok) {
				return {
					failed: true,
					outputData: {
						error: api.createError(evaluation.error, "COLOR_MATCH_INVALID", "validation"),
					},
				};
			}
			return { failed: false, outputData: evaluation.value };
		},
		describe: ({ api, context, failed, node }) => {
			const output = context.nodeOutputs[node.id];
			if (failed) {
				const message =
					typeof output?.error === "object" && output.error !== null && !Array.isArray(output.error)
						? String(output.error.message ?? "Color comparison failed.")
						: "Color comparison failed.";
				return [{ level: "error", message: `[Simulation] Color Match (${node.id}) failed: ${message}` }];
			}
			return [
				{
					level: "info",
					message: `[Simulation] Color Match (${node.id}) selected ${output?.matches === true ? "match" : "no match"} at ${api.formatValue(output?.difference_percent ?? 0)}% difference.`,
				},
			];
		},
	},
});

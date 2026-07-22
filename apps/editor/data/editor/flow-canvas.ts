import type { DefaultEdgeOptions } from "@xyflow/react";

export const edgeStyleOptions = [
	{ label: "Bezier", value: "bezier" },
	{ label: "Smooth step", value: "smoothstep" },
	{ label: "Step", value: "step" },
	{ label: "Straight", value: "straight" },
] as const;

export type EditorEdgeStyle = (typeof edgeStyleOptions)[number]["value"];

export const defaultEditorEdgeStyle: EditorEdgeStyle = "bezier";

export const editorEdgeZIndex = 10;

export const edgeColors = {
	default: "#53627d",
	selected: "#e62d3e",
} as const;

export function createDefaultEdgeOptions(edgeStyle: EditorEdgeStyle): DefaultEdgeOptions {
	return {
		type: toReactFlowEdgeType(edgeStyle),
		style: { stroke: edgeColors.default, strokeWidth: 2 },
		zIndex: editorEdgeZIndex,
	};
}

export function isEditorEdgeStyle(value: string): value is EditorEdgeStyle {
	return edgeStyleOptions.some((option) => option.value === value);
}

export const defaultEdgeOptions: DefaultEdgeOptions = {
	type: toReactFlowEdgeType(defaultEditorEdgeStyle),
	style: { stroke: edgeColors.default, strokeWidth: 2 },
	zIndex: editorEdgeZIndex,
};

export function toReactFlowEdgeType(edgeStyle: EditorEdgeStyle) {
	return edgeStyle === "bezier" ? "default" : edgeStyle;
}

export const reactFlowProOptions = { hideAttribution: true };

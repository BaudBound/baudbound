import type { DefaultEdgeOptions } from "@xyflow/react";

export const edgeColors = {
	default: "#53627d",
	selected: "#e62d3e",
} as const;

export const defaultEdgeOptions: DefaultEdgeOptions = {
	type: "smoothstep",
	style: { stroke: edgeColors.default, strokeWidth: 2 },
};

export const reactFlowProOptions = { hideAttribution: true };

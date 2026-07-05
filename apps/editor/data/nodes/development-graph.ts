import type { Edge, Node, XYPosition } from "@xyflow/react";
import type { ScriptNodeData } from "@/lib/types";
import { createNodeFromPaletteItem, getFlatPaletteItems } from "./registry";

export const isDevelopmentGraphEnabled = process.env.NODE_ENV === "development";

export function createInitialEditorNodes(): Node<ScriptNodeData>[] {
	return [];
}

export function createDevelopmentEditorNodes(center?: XYPosition): Node<ScriptNodeData>[] {
	if (!isDevelopmentGraphEnabled) {
		return [];
	}

	const paletteItems = getFlatPaletteItems();
	const columns = 5;
	const columnGap = 300;
	const rowGap = 200;
	const totalRows = Math.ceil(paletteItems.length / columns);
	const gridWidth = (Math.min(columns, paletteItems.length) - 1) * columnGap;
	const gridHeight = Math.max(0, totalRows - 1) * rowGap;
	const baseX = center ? center.x - gridWidth / 2 : 96;
	const baseY = center ? center.y - gridHeight / 2 : 80;

	return paletteItems.map((item, index) =>
		createNodeFromPaletteItem(item, index, {
			idPrefix: "dev-seed",
			baseX,
			baseY,
			columns,
			columnGap,
			rowGap,
		}),
	);
}

export function createInitialEditorEdges(): Edge[] {
	return [];
}

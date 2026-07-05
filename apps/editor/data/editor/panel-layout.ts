export type ResizablePanel = "left" | "right" | "bottom";

export type EditorPanelSizes = {
	left: number;
	right: number;
	bottom: number;
};

export const defaultPanelSizes: EditorPanelSizes = {
	left: 256,
	right: 320,
	bottom: 160,
};

export const panelSizeConstraints: Record<ResizablePanel, { min: number; max: number }> = {
	left: { min: 220, max: 420 },
	right: { min: 280, max: 520 },
	bottom: { min: 96, max: 560 },
};

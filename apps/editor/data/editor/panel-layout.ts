export type ResizablePanel = "left" | "right" | "bottom";

export type EditorPanelSizes = {
	left: number;
	right: number;
	bottom: number;
};

export type EditorPanelCollapsedState = Record<ResizablePanel, boolean>;

export const defaultPanelSizes: EditorPanelSizes = {
	left: 256,
	right: 320,
	bottom: 160,
};

export const defaultPanelCollapsedState: EditorPanelCollapsedState = {
	left: false,
	right: false,
	bottom: false,
};

export const collapsedPanelSizes: EditorPanelSizes = {
	left: 40,
	right: 40,
	bottom: 36,
};

export const panelSizeConstraints: Record<ResizablePanel, { min: number; max: number }> = {
	left: { min: 220, max: 420 },
	right: { min: 280, max: 520 },
	bottom: { min: 96, max: 560 },
};

export const responsivePanelLayout = {
	bottomResizeHandleHeight: 4,
	compactLeftMin: 148,
	compactRightMin: 196,
	minCanvasHeight: 220,
	minCanvasWidth: 320,
	resizeHandleWidth: 4,
	statusBarHeight: 24,
	topBarHeight: 48,
};

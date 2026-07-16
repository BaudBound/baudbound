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

export function sanitizePanelSizes(value: unknown): EditorPanelSizes {
	if (!isPanelSizesRecord(value)) {
		return defaultPanelSizes;
	}

	return {
		left: clamp(value.left, panelSizeConstraints.left.min, panelSizeConstraints.left.max),
		right: clamp(value.right, panelSizeConstraints.right.min, panelSizeConstraints.right.max),
		bottom: clamp(value.bottom, panelSizeConstraints.bottom.min, panelSizeConstraints.bottom.max),
	};
}

export function sanitizePanelCollapsedState(value: unknown): EditorPanelCollapsedState {
	if (!isPanelCollapsedState(value)) {
		return defaultPanelCollapsedState;
	}

	return {
		left: value.left,
		right: value.right,
		bottom: value.bottom,
	};
}

function isPanelSizesRecord(value: unknown): value is EditorPanelSizes {
	return (
		typeof value === "object" &&
		value !== null &&
		typeof (value as Partial<EditorPanelSizes>).left === "number" &&
		typeof (value as Partial<EditorPanelSizes>).right === "number" &&
		typeof (value as Partial<EditorPanelSizes>).bottom === "number"
	);
}

function isPanelCollapsedState(value: unknown): value is EditorPanelCollapsedState {
	return (
		typeof value === "object" &&
		value !== null &&
		typeof (value as Partial<EditorPanelCollapsedState>).left === "boolean" &&
		typeof (value as Partial<EditorPanelCollapsedState>).right === "boolean" &&
		typeof (value as Partial<EditorPanelCollapsedState>).bottom === "boolean"
	);
}

function clamp(value: number, min: number, max: number) {
	return Math.min(Math.max(value, min), max);
}

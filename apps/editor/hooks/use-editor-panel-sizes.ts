"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import {
	collapsedPanelSizes,
	defaultPanelCollapsedState,
	defaultPanelSizes,
	type EditorPanelCollapsedState,
	type EditorPanelSizes,
	panelSizeConstraints,
	type ResizablePanel,
	responsivePanelLayout,
	sanitizePanelSizes,
} from "@/data/editor/panel-layout";
import { getPanelPreferences, setPanelPreferences } from "@/data/storage/preference-repository";

const viewportFallback = {
	height: 900,
	width: 1440,
};

export function useEditorPanelSizes() {
	const activeResizeCleanupRef = useRef<(() => void) | null>(null);
	const [preferredSizes, setPreferredSizes] = useState<EditorPanelSizes>(defaultPanelSizes);
	const [collapsed, setCollapsed] = useState<EditorPanelCollapsedState>(defaultPanelCollapsedState);
	const [storageReady, setStorageReady] = useState(false);
	const [viewportSize, setViewportSize] = useState(viewportFallback);
	const sizes = fitPanelSizesToViewport(preferredSizes, collapsed, viewportSize);

	useEffect(() => {
		let active = true;
		void getPanelPreferences()
			.then((preferences) => {
				if (!active) return;
				setPreferredSizes(preferences.sizes);
				setCollapsed(preferences.collapsed);
			})
			.finally(() => {
				if (active) setStorageReady(true);
			});
		return () => {
			active = false;
		};
	}, []);

	useEffect(() => {
		let animationFrame = 0;

		const syncViewportSize = () => {
			cancelAnimationFrame(animationFrame);
			animationFrame = window.requestAnimationFrame(() => {
				setViewportSize({
					height: window.innerHeight,
					width: window.innerWidth,
				});
			});
		};

		syncViewportSize();
		window.addEventListener("resize", syncViewportSize);

		return () => {
			cancelAnimationFrame(animationFrame);
			window.removeEventListener("resize", syncViewportSize);
		};
	}, []);

	useEffect(() => {
		if (!storageReady) {
			return;
		}

		void setPanelPreferences({ collapsed, sizes: sanitizePanelSizes(preferredSizes) }).catch(() => {
			// Panel controls remain available when browser persistence is unavailable.
		});
	}, [collapsed, preferredSizes, storageReady]);

	useEffect(() => {
		return () => {
			activeResizeCleanupRef.current?.();
			activeResizeCleanupRef.current = null;
		};
	}, []);

	const startResize = useCallback(
		(panel: ResizablePanel, event: React.PointerEvent) => {
			event.preventDefault();
			activeResizeCleanupRef.current?.();

			const startX = event.clientX;
			const startY = event.clientY;
			const startSizes = preferredSizes;
			const cursor = panel === "bottom" ? "ns-resize" : "ew-resize";

			document.body.style.cursor = cursor;
			document.body.style.userSelect = "none";

			const handlePointerMove = (moveEvent: PointerEvent) => {
				setPreferredSizes(() => {
					if (panel === "left") {
						return {
							...startSizes,
							left: clamp(
								startSizes.left + moveEvent.clientX - startX,
								panelSizeConstraints.left.min,
								panelSizeConstraints.left.max,
							),
						};
					}

					if (panel === "right") {
						return {
							...startSizes,
							right: clamp(
								startSizes.right - (moveEvent.clientX - startX),
								panelSizeConstraints.right.min,
								panelSizeConstraints.right.max,
							),
						};
					}

					return {
						...startSizes,
						bottom: clamp(
							startSizes.bottom - (moveEvent.clientY - startY),
							panelSizeConstraints.bottom.min,
							panelSizeConstraints.bottom.max,
						),
					};
				});
			};

			const finishResize = () => {
				document.body.style.cursor = "";
				document.body.style.userSelect = "";
				window.removeEventListener("pointermove", handlePointerMove);
				window.removeEventListener("pointerup", finishResize);
				window.removeEventListener("pointercancel", finishResize);
				window.removeEventListener("blur", finishResize);
				activeResizeCleanupRef.current = null;
			};

			window.addEventListener("pointermove", handlePointerMove);
			window.addEventListener("pointerup", finishResize);
			window.addEventListener("pointercancel", finishResize);
			window.addEventListener("blur", finishResize);
			activeResizeCleanupRef.current = finishResize;
		},
		[preferredSizes],
	);
	const togglePanel = useCallback((panel: ResizablePanel) => {
		setCollapsed((current) => ({ ...current, [panel]: !current[panel] }));
	}, []);
	const expandPanel = useCallback((panel: ResizablePanel) => {
		setCollapsed((current) => (current[panel] ? { ...current, [panel]: false } : current));
	}, []);

	return { collapsed, expandPanel, sizes, startResize, togglePanel };
}

function clamp(value: number, min: number, max: number) {
	return Math.min(Math.max(value, min), max);
}

function fitPanelSizesToViewport(
	sizes: EditorPanelSizes,
	collapsed: EditorPanelCollapsedState,
	viewport: { height: number; width: number },
): EditorPanelSizes {
	const horizontalResizeHandleCount = Number(!collapsed.left) + Number(!collapsed.right);
	const horizontalRoom = Math.max(
		0,
		viewport.width -
			horizontalResizeHandleCount * responsivePanelLayout.resizeHandleWidth -
			responsivePanelLayout.minCanvasWidth,
	);
	const fittedSidePanels = fitSidePanels(sizes.left, sizes.right, collapsed, horizontalRoom);
	const maxBottom = Math.max(
		panelSizeConstraints.bottom.min,
		Math.min(
			panelSizeConstraints.bottom.max,
			viewport.height -
				responsivePanelLayout.topBarHeight -
				responsivePanelLayout.statusBarHeight -
				responsivePanelLayout.bottomResizeHandleHeight -
				responsivePanelLayout.minCanvasHeight,
		),
	);

	return {
		left: fittedSidePanels.left,
		right: fittedSidePanels.right,
		bottom: clamp(sizes.bottom, panelSizeConstraints.bottom.min, maxBottom),
	};
}

function fitSidePanels(left: number, right: number, collapsed: EditorPanelCollapsedState, availableWidth: number) {
	const collapsedWidth =
		Number(collapsed.left) * collapsedPanelSizes.left + Number(collapsed.right) * collapsedPanelSizes.right;
	const availableExpandedWidth = Math.max(0, availableWidth - collapsedWidth);

	if (collapsed.left && collapsed.right) {
		return { left: collapsedPanelSizes.left, right: collapsedPanelSizes.right };
	}

	if (collapsed.left) {
		return {
			left: collapsedPanelSizes.left,
			right: fitSingleSidePanel(right, "right", availableExpandedWidth),
		};
	}

	if (collapsed.right) {
		return {
			left: fitSingleSidePanel(left, "left", availableExpandedWidth),
			right: collapsedPanelSizes.right,
		};
	}

	return fitExpandedSidePanels(left, right, availableExpandedWidth);
}

function fitSingleSidePanel(value: number, panel: "left" | "right", availableWidth: number) {
	const desired = clamp(value, panelSizeConstraints[panel].min, panelSizeConstraints[panel].max);
	const absoluteMin = panel === "left" ? 96 : 128;

	return Math.max(absoluteMin, Math.min(desired, availableWidth));
}

function fitExpandedSidePanels(left: number, right: number, availableWidth: number) {
	const desiredLeft = clamp(left, panelSizeConstraints.left.min, panelSizeConstraints.left.max);
	const desiredRight = clamp(right, panelSizeConstraints.right.min, panelSizeConstraints.right.max);
	const desiredTotal = desiredLeft + desiredRight;

	if (desiredTotal <= availableWidth) {
		return { left: desiredLeft, right: desiredRight };
	}

	const compactTotal = responsivePanelLayout.compactLeftMin + responsivePanelLayout.compactRightMin;
	if (compactTotal <= availableWidth) {
		const shrinkableLeft = desiredLeft - responsivePanelLayout.compactLeftMin;
		const shrinkableRight = desiredRight - responsivePanelLayout.compactRightMin;
		const shrinkableTotal = shrinkableLeft + shrinkableRight;
		const overflow = desiredTotal - availableWidth;

		if (shrinkableTotal <= 0) {
			return {
				left: responsivePanelLayout.compactLeftMin,
				right: responsivePanelLayout.compactRightMin,
			};
		}

		return {
			left: Math.round(desiredLeft - overflow * (shrinkableLeft / shrinkableTotal)),
			right: Math.round(desiredRight - overflow * (shrinkableRight / shrinkableTotal)),
		};
	}

	const scale = availableWidth > 0 ? availableWidth / compactTotal : 0;

	return {
		left: Math.max(96, Math.floor(responsivePanelLayout.compactLeftMin * scale)),
		right: Math.max(128, Math.floor(responsivePanelLayout.compactRightMin * scale)),
	};
}

"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import {
	defaultPanelSizes,
	type EditorPanelSizes,
	panelSizeConstraints,
	type ResizablePanel,
} from "@/data/editor/panel-layout";

const PANEL_SIZES_STORAGE_KEY = "baudbound.editor.panel-sizes.v1";

export function useEditorPanelSizes() {
	const activeResizeCleanupRef = useRef<(() => void) | null>(null);
	const [sizes, setSizes] = useState<EditorPanelSizes>(defaultPanelSizes);
	const [storageReady, setStorageReady] = useState(false);

	useEffect(() => {
		setSizes(getStoredPanelSizes());
		setStorageReady(true);
	}, []);

	useEffect(() => {
		if (!storageReady) {
			return;
		}

		try {
			window.localStorage.setItem(PANEL_SIZES_STORAGE_KEY, JSON.stringify(sanitizePanelSizes(sizes)));
		} catch {
			// Local storage can be disabled or unavailable in private contexts; resizing should still work.
		}
	}, [sizes, storageReady]);

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
			const startSizes = sizes;
			const cursor = panel === "bottom" ? "ns-resize" : "ew-resize";

			document.body.style.cursor = cursor;
			document.body.style.userSelect = "none";

			const handlePointerMove = (moveEvent: PointerEvent) => {
				setSizes(() => {
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
		[sizes],
	);

	return { sizes, startResize };
}

function getStoredPanelSizes() {
	if (typeof window === "undefined") {
		return defaultPanelSizes;
	}

	try {
		const storedValue = window.localStorage.getItem(PANEL_SIZES_STORAGE_KEY);
		if (!storedValue) {
			return defaultPanelSizes;
		}

		return sanitizePanelSizes(JSON.parse(storedValue));
	} catch {
		return defaultPanelSizes;
	}
}

function sanitizePanelSizes(value: unknown): EditorPanelSizes {
	if (!isPanelSizesRecord(value)) {
		return defaultPanelSizes;
	}

	return {
		left: clamp(value.left, panelSizeConstraints.left.min, panelSizeConstraints.left.max),
		right: clamp(value.right, panelSizeConstraints.right.min, panelSizeConstraints.right.max),
		bottom: clamp(value.bottom, panelSizeConstraints.bottom.min, panelSizeConstraints.bottom.max),
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

function clamp(value: number, min: number, max: number) {
	return Math.min(Math.max(value, min), max);
}

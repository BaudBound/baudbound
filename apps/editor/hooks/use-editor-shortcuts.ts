"use client";

import { useEffect } from "react";
import { hasBrowserTextSelection, isEditableShortcutTarget } from "@/utils/editor-shortcuts";

export function useEditorShortcuts({
	onCopy,
	onRedo,
	onSave,
	onUndo,
}: {
	onCopy: () => boolean;
	onRedo: () => void;
	onSave: () => void;
	onUndo: () => void;
}) {
	useEffect(() => {
		const handleKeyDown = (event: KeyboardEvent) => {
			if (!(event.ctrlKey || event.metaKey)) return;

			const key = event.key.toLowerCase();
			if (key === "s") {
				event.preventDefault();
				onSave();
				return;
			}
			if (isEditableShortcutTarget(event.target)) return;
			if (key === "z") {
				event.preventDefault();
				if (event.shiftKey) onRedo();
				else onUndo();
				return;
			}
			if (key === "y") {
				event.preventDefault();
				onRedo();
				return;
			}
			if (key === "c" && !hasBrowserTextSelection() && onCopy()) {
				event.preventDefault();
			}
		};

		window.addEventListener("keydown", handleKeyDown);
		return () => window.removeEventListener("keydown", handleKeyDown);
	}, [onCopy, onRedo, onSave, onUndo]);
}

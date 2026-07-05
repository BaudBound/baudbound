export function hasBrowserTextSelection() {
	const selection = window.getSelection();

	return Boolean(selection && !selection.isCollapsed && selection.toString().length > 0);
}

export function isEditableShortcutTarget(target: EventTarget | null) {
	if (!(target instanceof HTMLElement)) {
		return false;
	}

	return Boolean(target.closest("input, textarea, select, [contenteditable='true'], [role='textbox']"));
}

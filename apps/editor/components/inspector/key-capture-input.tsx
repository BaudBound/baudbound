import type { KeyboardEvent } from "react";
import { useId } from "react";
import { Input } from "@/components/ui/input";

type KeyCaptureInputProps = {
	label: string;
	value: string;
	onChange: (value: string) => void;
};

export function KeyCaptureInput({ label, value, onChange }: KeyCaptureInputProps) {
	const inputId = useId();
	const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
		const capturedKey = getCapturedKey(event, value);
		if (!capturedKey) {
			return;
		}

		event.preventDefault();
		onChange(capturedKey);
	};

	return (
		<div>
			<label htmlFor={inputId} className="mb-1 block font-mono text-sm text-baud-muted">
				{label}
			</label>
			<Input
				id={inputId}
				value={value}
				onChange={(event) => onChange(normalizeManualKeyInput(event.target.value))}
				onKeyDown={handleKeyDown}
				placeholder="Press a shortcut or type one manually"
			/>
		</div>
	);
}

function getCapturedKey(event: KeyboardEvent<HTMLInputElement>, currentValue: string) {
	if (isModifierKey(event.key)) {
		return "";
	}

	if ((event.key === "Backspace" || event.key === "Delete") && currentValue.length > 0) {
		return "";
	}

	const keyName = normalizePressedKey(event.key, event.code);
	const hasModifier = event.ctrlKey || event.altKey || event.shiftKey || event.metaKey;
	const shouldCapture = hasModifier || isSpecialKey(keyName);
	if (!shouldCapture) {
		return "";
	}

	const parts = [
		event.ctrlKey ? "Ctrl" : "",
		event.altKey ? "Alt" : "",
		event.shiftKey ? "Shift" : "",
		event.metaKey ? "Meta" : "",
		keyName,
	].filter(Boolean);

	return parts.join("+");
}

function normalizePressedKey(key: string, code: string) {
	if (code.startsWith("Numpad")) {
		return code;
	}

	if (key === " ") {
		return "Space";
	}

	if (key.length === 1) {
		return key.toUpperCase();
	}

	return key;
}

function normalizeManualKeyInput(value: string) {
	return value.length === 1 ? value.toUpperCase() : value;
}

function isModifierKey(key: string) {
	return key === "Control" || key === "Alt" || key === "Shift" || key === "Meta";
}

function isSpecialKey(key: string) {
	return key.length > 1;
}

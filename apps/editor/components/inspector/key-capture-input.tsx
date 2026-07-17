import type { KeyboardEvent } from "react";
import { useId, useRef } from "react";
import { Input } from "@/components/ui/input";
import { canonicalWindowsKey } from "@/data/nodes/windows-key-contract";

type KeyCaptureInputProps = {
	label: string;
	value: string;
	onChange: (value: string) => void;
};

export function KeyCaptureInput({ label, value, onChange }: KeyCaptureInputProps) {
	const inputId = useId();
	const pressedKeys = useRef<string[]>([]);
	const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
		if ((event.key === "Backspace" || event.key === "Delete") && pressedKeys.current.length === 0 && value.length > 0) {
			return;
		}

		const keyName = canonicalWindowsKey(event.key, event.code);
		if (!keyName) return;
		event.preventDefault();
		if (event.repeat || pressedKeys.current.includes(keyName)) return;

		pressedKeys.current = [...pressedKeys.current, keyName];
		onChange(formatCapturedKeys(pressedKeys.current));
	};
	const handleKeyUp = (event: KeyboardEvent<HTMLInputElement>) => {
		const keyName = canonicalWindowsKey(event.key, event.code);
		if (!keyName) return;
		pressedKeys.current = pressedKeys.current.filter((pressedKey) => pressedKey !== keyName);
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
				onKeyUp={handleKeyUp}
				onBlur={() => {
					pressedKeys.current = [];
				}}
				placeholder="Press a key combination or use the buttons below"
			/>
		</div>
	);
}

function formatCapturedKeys(keys: string[]) {
	const modifiers = ["Ctrl", "Alt", "Shift", "Windows"];
	return [
		...modifiers.filter((modifier) => keys.includes(modifier)),
		...keys.filter((key) => !modifiers.includes(key)),
	].join("+");
}

function normalizeManualKeyInput(value: string) {
	return value.length === 1 ? value.toUpperCase() : value;
}

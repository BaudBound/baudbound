import { useRef, type KeyboardEvent } from "react";

import windowsKeyContract from "../../../contracts/runner/windows-keyboard-keys.json";
import { Input } from "@/components/ui/input";

const modifierAliases = new Map(
  windowsKeyContract.modifiers.flatMap((modifier) =>
    [modifier.canonical, ...modifier.aliases].map((alias) => [
      normalizeToken(alias),
      modifier.canonical,
    ]),
  ),
);
const keyAliases = new Map(
  windowsKeyContract.keys.flatMap((key) =>
    [key.canonical, ...key.aliases].map((alias) => [normalizeToken(alias), key.canonical]),
  ),
);

export function HotkeyInput({
  id,
  onChange,
  value,
}: {
  id: string;
  onChange: (value: string) => void;
  value: string;
}) {
  const pressedKeys = useRef<string[]>([]);

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (
      (event.key === "Backspace" || event.key === "Delete") &&
      pressedKeys.current.length === 0 &&
      value.length > 0
    ) {
      return;
    }
    const keyName = canonicalWindowsKey(event.key, event.code);
    if (!keyName) return;
    event.preventDefault();
    if (event.repeat || pressedKeys.current.includes(keyName)) return;

    pressedKeys.current = [...pressedKeys.current, keyName];
    onChange(formatCapturedKeys(pressedKeys.current));
  };

  return (
    <Input
      id={id}
      onBlur={() => {
        pressedKeys.current = [];
      }}
      onChange={(event) => onChange(normalizeManualKeyInput(event.target.value))}
      onKeyDown={handleKeyDown}
      onKeyUp={(event) => {
        const keyName = canonicalWindowsKey(event.key, event.code);
        pressedKeys.current = pressedKeys.current.filter((pressedKey) => pressedKey !== keyName);
      }}
      placeholder="Press a key combination"
      value={value}
    />
  );
}

export function validateHotkey(value: string) {
  const parts = value.split(/[+-]/).map((part) => part.trim());
  if (parts.length === 0 || parts.some((part) => !part)) return false;
  const seen = new Set<string>();
  return parts.every((part) => {
    const canonical =
      modifierAliases.get(normalizeToken(part)) ?? keyAliases.get(normalizeToken(part));
    if (!canonical || seen.has(canonical)) return false;
    seen.add(canonical);
    return true;
  });
}

function canonicalWindowsKey(key: string, code: string) {
  const browserKey = key === "Meta" || key === "OS" ? "Windows" : key;
  const modifier = modifierAliases.get(normalizeToken(browserKey));
  if (modifier) return modifier;
  const candidate = browserCodeCandidate(code);
  return keyAliases.get(normalizeToken(candidate || (key === " " ? "Space" : key))) ?? "";
}

function browserCodeCandidate(code: string) {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit[0-9]$/.test(code)) return code.slice(5);
  if (code === "NumpadEnter") return "Enter";
  if (code === "NumpadComma") return "NumpadSeparator";
  return code;
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

function normalizeToken(value: string) {
  return value.trim().toLowerCase().replace(/[ _]/g, "");
}

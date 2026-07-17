import windowsKeyContract from "./windows-key-contract.json";

const modifierAliases = new Map(
	windowsKeyContract.modifiers.flatMap((modifier) =>
		[modifier.canonical, ...modifier.aliases].map((alias) => [normalizeToken(alias), modifier.canonical] as const),
	),
);
const keyAliases = new Map(
	windowsKeyContract.keys.flatMap((key) =>
		[key.canonical, ...key.aliases].map((alias) => [normalizeToken(alias), key.canonical] as const),
	),
);

export function validateWindowsKeyExpression(expression: string) {
	const parts = expression.split(/[+-]/).map((part) => part.trim());
	if (parts.length === 0 || parts.some((part) => part.length === 0)) {
		return "key expression must contain at least one supported key.";
	}

	const seenKeys = new Set<string>();
	for (const part of parts) {
		const normalized = normalizeToken(part);
		const canonical = modifierAliases.get(normalized) ?? keyAliases.get(normalized);
		if (!canonical) {
			return `${part} is not supported. Use a key listed in the Windows key reference.`;
		}
		if (seenKeys.has(canonical)) {
			return `key expression contains ${canonical} more than once.`;
		}
		seenKeys.add(canonical);
	}

	return "";
}

export const validateWindowsHotkey = validateWindowsKeyExpression;

export function canonicalWindowsKey(key: string, code: string) {
	const browserKey = key === "Meta" || key === "OS" ? "Windows" : key;
	const modifierCandidate = modifierAliases.get(normalizeToken(browserKey));
	if (modifierCandidate) {
		return modifierCandidate;
	}
	const codeCandidate = browserCodeCandidate(code);
	if (codeCandidate) {
		return keyAliases.get(normalizeToken(codeCandidate)) ?? "";
	}
	return keyAliases.get(normalizeToken(key === " " ? "Space" : key)) ?? "";
}

function browserCodeCandidate(code: string) {
	if (/^Key[A-Z]$/.test(code)) {
		return code.slice(3);
	}
	if (/^Digit[0-9]$/.test(code)) {
		return code.slice(5);
	}
	if (code === "NumpadEnter") {
		return "Enter";
	}
	if (code === "NumpadComma") {
		return "NumpadSeparator";
	}
	return code;
}

function normalizeToken(value: string) {
	return value.trim().toLowerCase().replace(/[ _]/g, "");
}

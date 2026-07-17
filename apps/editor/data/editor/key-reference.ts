import windowsKeyContract from "../nodes/windows-key-contract.json";

export type KeyReferenceGroup = {
	label: string;
	keys: string[];
};

const punctuationKeys = new Set([
	"Semicolon",
	"Equal",
	"Comma",
	"Minus",
	"Period",
	"Slash",
	"Backquote",
	"BracketLeft",
	"Backslash",
	"BracketRight",
	"Quote",
	"IntlBackslash",
]);

const systemKeys = new Set(["CapsLock", "NumLock", "ScrollLock", "PrintScreen", "Pause", "ContextMenu"]);

const groupOrder = [
	"Letters and digits",
	"Function",
	"Navigation and editing",
	"System and lock",
	"Punctuation",
	"Numpad",
	"Browser",
	"Media and volume",
	"Application launch",
] as const;

export const windowsKeyboardKeyReference: KeyReferenceGroup[] = [
	{
		label: "Modifiers",
		keys: windowsKeyContract.modifiers.map((modifier) => modifier.canonical),
	},
	...groupOrder.map((label) => ({
		label,
		keys: windowsKeyContract.keys.filter((key) => getKeyGroup(key.canonical) === label).map((key) => key.canonical),
	})),
];

function getKeyGroup(key: string): (typeof groupOrder)[number] {
	if (/^[A-Z0-9]$/.test(key)) return "Letters and digits";
	if (/^F(?:[1-9]|1[0-9]|2[0-4])$/.test(key)) return "Function";
	if (punctuationKeys.has(key)) return "Punctuation";
	if (systemKeys.has(key)) return "System and lock";
	if (key.startsWith("Numpad")) return "Numpad";
	if (key.startsWith("Browser")) return "Browser";
	if (key.startsWith("Media") || key.startsWith("Volume")) return "Media and volume";
	if (key.startsWith("Launch")) return "Application launch";
	return "Navigation and editing";
}

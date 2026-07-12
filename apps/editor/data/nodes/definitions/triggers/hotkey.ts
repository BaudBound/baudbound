import { Keyboard } from "lucide-react";
import { defineNode } from "../../node-definition";
import { configString, requiredStaticConfig } from "../validators";

export const hotkeyTriggerNode = defineNode({
	actionType: "trigger.hotkey",
	capabilities: ["trigger.hotkey"],
	configFields: [{ key: "key", label: "Key", type: "text" }],
	defaultConfig: () => ({ key: "Ctrl+Alt+B" }),
	description: "Start from a desktop hotkey.",
	desktopOnly: true,
	group: "triggers",
	icon: Keyboard,
	kind: "trigger",
	label: "Hotkey",
	risk: "medium",
	runtimeOutputs: [
		{
			name: "key",
			type: "keyboard_key",
			description: "Captured hotkey expression.",
			example: "n-mr3zyt6f-4.key",
		},
	],
	runnerType: "hotkey",
	supportedTargetRuntimes: ["Windows Desktop"],
	validateConfig: (config) => {
		const requiredError = requiredStaticConfig(config, "key", "hotkey");
		if (requiredError) {
			return [requiredError];
		}

		const error = validateWindowsHotkey(configString(config, "key"));
		return error ? [error] : [];
	},
	simulation: {
		createOutput: ({ api, context, node }) => ({
			failed: false,
			outputData: { key: context.triggerPayload.key || api.getConfigString(node, "key") },
		}),
	},
});

function validateWindowsHotkey(expression: string) {
	const modifiers = new Set([
		"ctrl",
		"control",
		"alt",
		"option",
		"shift",
		"meta",
		"cmd",
		"command",
		"win",
		"windows",
		"super",
	]);
	const keys = expression
		.split(/[+-]/)
		.map((part) => part.trim())
		.filter(Boolean)
		.filter((part) => !modifiers.has(part.toLowerCase()));

	if (keys.length !== 1) {
		return "hotkey must include exactly one primary key.";
	}

	const key = keys[0].toLowerCase().replace(/[ _]/g, "");
	const namedKeys = new Set([
		"esc",
		"escape",
		"return",
		"enter",
		"space",
		"spacebar",
		"tab",
		"backspace",
		"delete",
		"del",
		"insert",
		"ins",
		"home",
		"end",
		"pageup",
		"pagedown",
		"up",
		"arrowup",
		"down",
		"arrowdown",
		"left",
		"arrowleft",
		"right",
		"arrowright",
	]);
	const isCharacter = /^[a-z0-9]$/.test(key);
	const functionKey = /^f([1-9]|1\d|2[0-4])$/.test(key);

	return isCharacter || functionKey || namedKeys.has(key)
		? ""
		: "hotkey key is not supported. Use A-Z, 0-9, F1-F24, or a navigation key.";
}

import { expect, test } from "@playwright/test";
import { executeTextTransform } from "../../data/nodes/definitions/actions/format-text";
import type { JsonValue } from "../../lib/types";

const variables: Record<string, JsonValue> = {
	paddingLength: 3,
};

const resolveTemplate = (value: string): JsonValue => {
	const reference = /^\{\{\s*([^{}]+?)\s*}}$/.exec(value);
	return reference ? (variables[reference[1]] ?? value) : value;
};

const parseJsonValue = (value: string): JsonValue | undefined => {
	try {
		return JSON.parse(value) as JsonValue;
	} catch {
		return undefined;
	}
};

test("editor executes every text transform operation", () => {
	const cases: Array<{
		config: Record<string, JsonValue>;
		expectedItems?: JsonValue[];
		expectedText: string;
	}> = [
		{ config: { operation: "template", template: "Hello Ada" }, expectedText: "Hello Ada" },
		{ config: { input: "  text \n", operation: "trim" }, expectedText: "text" },
		{ config: { input: "BaudBound", operation: "uppercase" }, expectedText: "BAUDBOUND" },
		{ config: { input: "BaudBound", operation: "lowercase" }, expectedText: "baudbound" },
		{ config: { input: "hELLO WORLD", operation: "sentence_case" }, expectedText: "Hello world" },
		{ config: { input: "hELLO   wORLD", operation: "capitalize_words" }, expectedText: "Hello   World" },
		{
			config: { input: "one one", operation: "replace", replacement: "two", search: "one" },
			expectedText: "two two",
		},
		{
			config: { input: "a1 b22", operation: "regex_replace", replacement: "#", search: "\\d+" },
			expectedText: "a# b#",
		},
		{
			config: { delimiter: ",", input: "one,two,three", operation: "split" },
			expectedItems: ["one", "two", "three"],
			expectedText: "",
		},
		{
			config: { delimiter: "|", items: '["one",2,true,{"ok":true}]', operation: "join" },
			expectedItems: ["one", 2, true, { ok: true }],
			expectedText: 'one|2|true|{"ok":true}',
		},
		{
			config: { input: "A😀BC", length: "2", operation: "substring", start: "1" },
			expectedText: "😀B",
		},
		{
			config: { input: "7", operation: "pad_start", pad: "0", targetLength: "{{paddingLength}}" },
			expectedText: "007",
		},
		{
			config: { input: "7", operation: "pad_end", pad: "0", targetLength: "{{paddingLength}}" },
			expectedText: "700",
		},
		{ config: { input: "a b&!'()*~", operation: "url_encode" }, expectedText: "a%20b%26!'()*~" },
		{ config: { input: "a%20b%26!'()*~", operation: "url_decode" }, expectedText: "a b&!'()*~" },
		{ config: { input: "BaudBound ✓", operation: "base64_encode" }, expectedText: "QmF1ZEJvdW5kIOKckw==" },
		{ config: { input: "QmF1ZEJvdW5kIOKckw==", operation: "base64_decode" }, expectedText: "BaudBound ✓" },
		{ config: { input: 'line\n"quoted"', operation: "json_escape" }, expectedText: '"line\\n\\"quoted\\""' },
		{ config: { input: '{"ok":true}', operation: "json_unescape" }, expectedText: '{"ok":true}' },
	];

	for (const { config, expectedItems = [], expectedText } of cases) {
		const result = executeTextTransform({ config, parseJsonValue, resolveTemplate });
		expect(result, `operation ${String(config.operation)} should succeed`).toEqual({
			ok: true,
			output: { items: expectedItems, text: expectedText },
		});
	}
});

test("editor rejects malformed text transform inputs", () => {
	const cases: Array<Record<string, JsonValue>> = [
		{ input: "text", operation: "regex_replace", replacement: "", search: "[" },
		{ delimiter: ",", items: "{}", operation: "join" },
		{ input: "%%%", operation: "base64_decode" },
		{ input: "YQ", operation: "base64_decode" },
		{ input: "Y Q==", operation: "base64_decode" },
		{ input: "/w==", operation: "base64_decode" },
		{ input: "%ZZ", operation: "url_decode" },
		{ input: "not-json", operation: "json_unescape" },
		{ input: "text", operation: "unsupported" },
	];

	for (const config of cases) {
		const result = executeTextTransform({ config, parseJsonValue, resolveTemplate });
		expect(result.ok, `operation ${String(config.operation)} should fail`).toBe(false);
	}
});

import { expect, test } from "@playwright/test";
import { parseAbsoluteUrl } from "../../data/nodes/definitions/actions/parse-url";

test("parses standard and custom absolute URLs", () => {
	expect(parseAbsoluteUrl("https://nat.gg:8443/test?param=value1&tag=one&tag=two#result")).toEqual({
		ok: true,
		output: {
			fragment: "result",
			host: "nat.gg",
			path: "/test",
			port: "8443",
			protocol: "https",
			query: "param=value1&tag=one&tag=two",
			query_parameters: [
				{ name: "param", value: "value1" },
				{ name: "tag", value: "one" },
				{ name: "tag", value: "two" },
			],
		},
	});

	expect(parseAbsoluteUrl("ptr://command/move?param=value1")).toEqual({
		ok: true,
		output: {
			fragment: "",
			host: "command",
			path: "/move",
			port: "",
			protocol: "ptr",
			query: "param=value1",
			query_parameters: [{ name: "param", value: "value1" }],
		},
	});
});

test("preserves duplicate parameters and decodes their names and values", () => {
	const result = parseAbsoluteUrl("custom://host/path?name=Baud%20Bound&tag=one&tag=two&empty");
	expect(result.ok).toBe(true);
	if (result.ok) {
		expect(result.output.query_parameters).toEqual([
			{ name: "name", value: "Baud Bound" },
			{ name: "tag", value: "one" },
			{ name: "tag", value: "two" },
			{ name: "empty", value: "" },
		]);
	}
});

test("rejects relative and malformed URLs", () => {
	for (const value of ["", "/relative/path?param=value", "https://[invalid"]) {
		expect(parseAbsoluteUrl(value).ok).toBe(false);
	}
});

import { describe, expect, it } from "vitest";

import {
	formDialogActionLabels,
	messageDialogButtonSets,
	messageDialogCloseButton,
	parseDesktopDialogPayload,
} from "./desktop-dialog-model";

const base = {
	description: "Description",
	dialogSize: "medium",
	kind: "form_dialog",
	requestingScript: "script-id",
	timeoutAtUnixMs: null,
	title: "Title",
};

const commonField = {
	description: "Field description",
	key: "result",
	label: "Result",
	required: true,
};

describe("desktop dialog payload parsing", () => {
	it("keeps form actions stable and rejects an empty form", () => {
		expect(formDialogActionLabels).toEqual({ cancel: "Cancel", submit: "Submit" });
		expect(() => parseDesktopDialogPayload({ ...base, fields: [] })).toThrow(/at least one form component/);
	});

	it("accepts every strict form component", () => {
		const fields = [
			{ ...commonField, defaultChecked: false, type: "checkbox" },
			{ ...commonField, defaultValue: "#12AB34", key: "color", type: "color" },
			{ ...commonField, defaultValue: "2026-08-03", key: "date", type: "date" },
			{
				...commonField,
				defaultValue: "2026-08-03T12:30:00",
				key: "datetime",
				timezone: "Europe/Helsinki",
				type: "datetime",
			},
			{ accentColor: "#5B8AF5", type: "divider" },
			{
				...commonField,
				choices: [{ displayValue: "Displayed", key: "output-key" }],
				key: "dropdown",
				type: "dropdown",
			},
			{ ...commonField, key: "file", multiple: true, type: "file" },
			{ ...commonField, key: "folder", type: "folder" },
			{
				dataUrl: "data:image/png;base64,AA==",
				description: "Caption",
				imageFit: "contain",
				imageHeight: 240,
				label: "Alternative text",
				type: "image",
			},
			{ accentColor: "#5B8AF5", description: "Read this", label: "Information", type: "information" },
			{
				...commonField,
				choices: [{ displayValue: "Displayed", key: "output-key" }],
				key: "multi",
				type: "multi_choice",
			},
			{ ...commonField, defaultValue: "Long text", key: "notes", placeholder: "Notes", type: "multiline" },
			{ ...commonField, defaultValue: 2.5, key: "amount", placeholder: "0", type: "number" },
			{ ...commonField, key: "secret", placeholder: "Password", type: "password" },
			{ accentColor: "#5B8AF5", description: "Section", label: "Heading", type: "section_heading" },
			{
				...commonField,
				choices: [{ displayValue: "Displayed", key: "output-key" }],
				key: "single",
				type: "single_choice",
			},
			{ ...commonField, defaultValue: "Default", key: "name", placeholder: "Name", type: "text" },
			{
				...commonField,
				defaultValue: 50,
				key: "slider",
				maximum: 100,
				minimum: 0,
				step: 5,
				type: "slider",
			},
			{ ...commonField, defaultValue: "12:30:00", key: "time", type: "time" },
		];

		expect(parseDesktopDialogPayload({ ...base, fields })).toMatchObject({ fields });
	});

	it("rejects cross-type fields and malformed choices", () => {
		expect(() =>
			parseDesktopDialogPayload({
				...base,
				fields: [
					{
						accentColor: "#5B8AF5",
						description: "",
						label: "Info",
						required: true,
						type: "information",
					},
				],
			}),
		).toThrow(/unexpected field "required"/);
		expect(() =>
			parseDesktopDialogPayload({
				...base,
				fields: [{ accentColor: "blue", description: "", label: "Info", type: "information" }],
			}),
		).toThrow(/normalized #RRGGBB color/);
		expect(() =>
			parseDesktopDialogPayload({
				...base,
				fields: [
					{
						...commonField,
						choices: [{ displayValue: "Displayed", key: 42 }],
						type: "single_choice",
					},
				],
			}),
		).toThrow(/field "key" must be text/);
		expect(() =>
			parseDesktopDialogPayload({
				...base,
				fields: [{ ...commonField, defaultValue: "not numeric", placeholder: "", type: "number" }],
			}),
		).toThrow(/must be a finite number or null/);
		expect(() =>
			parseDesktopDialogPayload({
				...base,
				dialogSize: "huge",
				fields: [{ ...commonField, defaultValue: "", placeholder: "", type: "text" }],
			}),
		).toThrow(/unsupported value "huge"/);
	});

	it("accepts strict message payloads and rejects unknown variants", () => {
		expect(
			parseDesktopDialogPayload({
				buttons: "yes_no",
				dialogSize: "large",
				kind: "message_dialog",
				message: "Continue?",
				requestingScript: "script-id",
				timeoutAtUnixMs: 1_785_776_400_000,
				title: "Question",
				variant: "warning",
			}),
		).toMatchObject({ buttons: "yes_no", dialogSize: "large", variant: "warning" });
		expect(
			parseDesktopDialogPayload({
				buttons: "cancel_confirm",
				dialogSize: "medium",
				kind: "message_dialog",
				message: "Apply these changes?",
				requestingScript: "script-id",
				timeoutAtUnixMs: null,
				title: "Confirmation",
				variant: "info",
			}),
		).toMatchObject({ buttons: "cancel_confirm" });
		expect(() =>
			parseDesktopDialogPayload({
				buttons: "maybe",
				dialogSize: "small",
				kind: "message_dialog",
				message: "Continue?",
				requestingScript: "script-id",
				timeoutAtUnixMs: null,
				title: "Question",
				variant: "warning",
			}),
		).toThrow(/unsupported value "maybe"/);
	});

	it("defines exact property-driven message button and close behavior", () => {
		expect(messageDialogButtonSets).toEqual({
			cancel_confirm: ["cancel", "confirm"],
			ok: ["ok"],
			ok_cancel: ["cancel", "ok"],
			yes_no: ["no", "yes"],
			yes_no_cancel: ["cancel", "no", "yes"],
		});
		expect(messageDialogCloseButton("cancel_confirm")).toBe("cancel");
		expect(messageDialogCloseButton("ok")).toBe("ok");
		expect(messageDialogCloseButton("yes_no")).toBeNull();
	});
});

export type MessageButtons = "cancel_confirm" | "ok" | "ok_cancel" | "yes_no" | "yes_no_cancel";
export type MessageDialogButton = "cancel" | "confirm" | "no" | "ok" | "yes";
export type MessageVariant = "error" | "info" | "warning";

export const messageDialogButtonSets = {
	cancel_confirm: ["cancel", "confirm"],
	ok: ["ok"],
	ok_cancel: ["cancel", "ok"],
	yes_no: ["no", "yes"],
	yes_no_cancel: ["cancel", "no", "yes"],
} as const satisfies Record<MessageButtons, readonly MessageDialogButton[]>;

const messageDialogCloseButtons = {
	cancel_confirm: "cancel",
	ok: "ok",
	ok_cancel: "cancel",
	yes_no: null,
	yes_no_cancel: "cancel",
} as const satisfies Record<MessageButtons, MessageDialogButton | null>;

export function messageDialogCloseButton(buttons: MessageButtons): MessageDialogButton | null {
	return messageDialogCloseButtons[buttons];
}

export const formDialogActionLabels = {
	cancel: "Cancel",
	submit: "Submit",
} as const;

export type DialogChoice = {
	displayValue: string;
	key: string;
};

type FormDialogFieldBase = {
	description: string;
	key: string;
	label: string;
	required: boolean;
};

export type FormDialogField =
	| (FormDialogFieldBase & { defaultChecked: boolean; type: "checkbox" })
	| (FormDialogFieldBase & { choices: DialogChoice[]; type: "multi_choice" })
	| (FormDialogFieldBase & { choices: DialogChoice[]; type: "single_choice" })
	| (FormDialogFieldBase & { choices: DialogChoice[]; type: "dropdown" })
	| (FormDialogFieldBase & { defaultValue: string; placeholder: string; type: "multiline" })
	| (FormDialogFieldBase & { defaultValue: string; placeholder: string; type: "text" })
	| (FormDialogFieldBase & { defaultValue: number | null; placeholder: string; type: "number" })
	| (FormDialogFieldBase & { placeholder: string; type: "password" })
	| (FormDialogFieldBase & { defaultValue: string; type: "color" })
	| (FormDialogFieldBase & { defaultValue: string; type: "date" })
	| (FormDialogFieldBase & { defaultValue: string; type: "time" })
	| (FormDialogFieldBase & { defaultValue: string; timezone: string; type: "datetime" })
	| (FormDialogFieldBase & { multiple: boolean; type: "file" })
	| (FormDialogFieldBase & { type: "folder" })
	| (FormDialogFieldBase & {
			defaultValue: number;
			maximum: number;
			minimum: number;
			step: number;
			type: "slider";
	  })
	| { accentColor: string; description: string; label: string; type: "information" }
	| { accentColor: string; description: string; label: string; type: "section_heading" }
	| { accentColor: string; type: "divider" }
	| {
			dataUrl: string;
			description: string;
			imageFit: "contain" | "cover";
			imageHeight: number;
			label: string;
			type: "image";
	  };

export type MessageDialogPayload = {
	buttons: MessageButtons;
	dialogSize: "large" | "medium" | "small";
	kind: "message_dialog";
	message: string;
	requestingScript: string;
	timeoutAtUnixMs: number | null;
	title: string;
	variant: MessageVariant;
};

export type FormDialogPayload = {
	description: string;
	dialogSize: "large" | "medium" | "small";
	fields: FormDialogField[];
	kind: "form_dialog";
	requestingScript: string;
	timeoutAtUnixMs: number | null;
	title: string;
};

export type DesktopDialogPayload = MessageDialogPayload | FormDialogPayload;

const messageButtons = new Set<MessageButtons>(Object.keys(messageDialogButtonSets) as MessageButtons[]);
const messageVariants = new Set<MessageVariant>(["error", "info", "warning"]);
const dialogSizes = new Set(["large", "medium", "small"] as const);
const formDialogFieldTypes = new Set([
	"checkbox",
	"color",
	"date",
	"datetime",
	"divider",
	"dropdown",
	"file",
	"folder",
	"image",
	"information",
	"multi_choice",
	"multiline",
	"number",
	"password",
	"single_choice",
	"section_heading",
	"slider",
	"text",
	"time",
] as const);

export function parseDesktopDialogPayload(value: unknown): DesktopDialogPayload {
	const payload = record(value, "desktop dialog payload");
	const kind = stringField(payload, "kind");
	if (kind === "message_dialog") {
		exactFields(payload, [
			"buttons",
			"dialogSize",
			"kind",
			"message",
			"requestingScript",
			"timeoutAtUnixMs",
			"title",
			"variant",
		]);
		return {
			buttons: enumField(payload, "buttons", messageButtons),
			dialogSize: enumField(payload, "dialogSize", dialogSizes),
			kind,
			message: stringField(payload, "message"),
			requestingScript: stringField(payload, "requestingScript"),
			timeoutAtUnixMs: nullableTimestampField(payload, "timeoutAtUnixMs"),
			title: stringField(payload, "title"),
			variant: enumField(payload, "variant", messageVariants),
		};
	}
	if (kind !== "form_dialog") {
		throw new Error(`desktop dialog payload has unsupported kind ${JSON.stringify(kind)}`);
	}

	exactFields(payload, ["description", "dialogSize", "fields", "kind", "requestingScript", "timeoutAtUnixMs", "title"]);
	if (!Array.isArray(payload.fields)) {
		throw new Error("desktop dialog fields must be a list");
	}
	if (payload.fields.length === 0) {
		throw new Error("desktop dialog requires at least one form component");
	}
	return {
		description: stringField(payload, "description"),
		dialogSize: enumField(payload, "dialogSize", dialogSizes),
		fields: payload.fields.map(parseFormDialogField),
		kind,
		requestingScript: stringField(payload, "requestingScript"),
		timeoutAtUnixMs: nullableTimestampField(payload, "timeoutAtUnixMs"),
		title: stringField(payload, "title"),
	};
}

function parseFormDialogField(value: unknown, index: number): FormDialogField {
	const field = record(value, `desktop dialog field ${index + 1}`);
	const type = enumField(field, "type", formDialogFieldTypes);
	if (type === "information" || type === "section_heading") {
		exactFields(field, ["accentColor", "description", "label", "type"]);
		return {
			accentColor: normalizedColorField(field, "accentColor"),
			description: stringField(field, "description"),
			label: stringField(field, "label"),
			type,
		};
	}
	if (type === "divider") {
		exactFields(field, ["accentColor", "type"]);
		return { accentColor: normalizedColorField(field, "accentColor"), type };
	}
	if (type === "image") {
		exactFields(field, ["dataUrl", "description", "imageFit", "imageHeight", "label", "type"]);
		const dataUrl = stringField(field, "dataUrl");
		if (
			dataUrl.length > 11_200_000 ||
			!/^data:image\/(?:png|jpeg|webp|gif|svg\+xml);base64,[A-Za-z0-9+/]+={0,2}$/.test(dataUrl)
		) {
			throw new Error("desktop dialog image source is invalid");
		}
		const imageHeight = numberField(field, "imageHeight");
		if (!Number.isInteger(imageHeight) || imageHeight < 80 || imageHeight > 600) {
			throw new Error("desktop dialog image height is invalid");
		}
		return {
			dataUrl,
			description: stringField(field, "description"),
			imageFit: enumField(field, "imageFit", new Set(["contain", "cover"] as const)),
			imageHeight,
			label: stringField(field, "label"),
			type,
		};
	}

	const base = {
		description: stringField(field, "description"),
		key: stringField(field, "key"),
		label: stringField(field, "label"),
		required: booleanField(field, "required"),
	};
	switch (type) {
		case "checkbox":
			exactFields(field, ["defaultChecked", "description", "key", "label", "required", "type"]);
			return { ...base, defaultChecked: booleanField(field, "defaultChecked"), type };
		case "multi_choice":
		case "single_choice":
		case "dropdown":
			exactFields(field, ["choices", "description", "key", "label", "required", "type"]);
			return { ...base, choices: parseChoices(field.choices, index), type };
		case "multiline":
		case "text":
			exactFields(field, ["defaultValue", "description", "key", "label", "placeholder", "required", "type"]);
			return {
				...base,
				defaultValue: stringField(field, "defaultValue"),
				placeholder: stringField(field, "placeholder"),
				type,
			};
		case "number":
			exactFields(field, ["defaultValue", "description", "key", "label", "placeholder", "required", "type"]);
			return {
				...base,
				defaultValue: nullableNumberField(field, "defaultValue"),
				placeholder: stringField(field, "placeholder"),
				type,
			};
		case "password":
			exactFields(field, ["description", "key", "label", "placeholder", "required", "type"]);
			return { ...base, placeholder: stringField(field, "placeholder"), type };
		case "color":
		case "date":
		case "time":
			exactFields(field, ["defaultValue", "description", "key", "label", "required", "type"]);
			return {
				...base,
				defaultValue:
					type === "color" ? normalizedColorField(field, "defaultValue") : stringField(field, "defaultValue"),
				type,
			};
		case "datetime":
			exactFields(field, ["defaultValue", "description", "key", "label", "required", "timezone", "type"]);
			return {
				...base,
				defaultValue: stringField(field, "defaultValue"),
				timezone: stringField(field, "timezone"),
				type,
			};
		case "file":
			exactFields(field, ["description", "key", "label", "multiple", "required", "type"]);
			return { ...base, multiple: booleanField(field, "multiple"), type };
		case "folder":
			exactFields(field, ["description", "key", "label", "required", "type"]);
			return { ...base, type };
		case "slider": {
			exactFields(field, [
				"defaultValue",
				"description",
				"key",
				"label",
				"maximum",
				"minimum",
				"required",
				"step",
				"type",
			]);
			const minimum = numberField(field, "minimum");
			const maximum = numberField(field, "maximum");
			const step = numberField(field, "step");
			const defaultValue = numberField(field, "defaultValue");
			if (maximum <= minimum || step <= 0 || defaultValue < minimum || defaultValue > maximum) {
				throw new Error("desktop dialog slider configuration is invalid");
			}
			return { ...base, defaultValue, maximum, minimum, step, type };
		}
	}
}

function parseChoices(value: unknown, fieldIndex: number): DialogChoice[] {
	if (!Array.isArray(value)) {
		throw new Error(`desktop dialog field ${fieldIndex + 1} choices must be a list`);
	}
	return value.map((choice, choiceIndex) => {
		const entry = record(choice, `desktop dialog field ${fieldIndex + 1} choice ${choiceIndex + 1}`);
		exactFields(entry, ["displayValue", "key"]);
		return {
			displayValue: stringField(entry, "displayValue"),
			key: stringField(entry, "key"),
		};
	});
}

function record(value: unknown, label: string): Record<string, unknown> {
	if (!value || typeof value !== "object" || Array.isArray(value)) {
		throw new Error(`${label} must be an object`);
	}
	return value as Record<string, unknown>;
}

function exactFields(value: Record<string, unknown>, fields: readonly string[]) {
	const allowed = new Set(fields);
	const unexpected = Object.keys(value).find((field) => !allowed.has(field));
	if (unexpected) {
		throw new Error(`desktop dialog payload contains unexpected field ${JSON.stringify(unexpected)}`);
	}
	const missing = fields.find((field) => !(field in value));
	if (missing) {
		throw new Error(`desktop dialog payload is missing field ${JSON.stringify(missing)}`);
	}
}

function stringField(value: Record<string, unknown>, field: string) {
	const result = value[field];
	if (typeof result !== "string") {
		throw new Error(`desktop dialog field ${JSON.stringify(field)} must be text`);
	}
	return result;
}

function nullableTimestampField(value: Record<string, unknown>, field: string) {
	const result = value[field];
	if (result === null) return null;
	if (!Number.isSafeInteger(result) || Number(result) <= 0) {
		throw new Error(`desktop dialog field ${JSON.stringify(field)} must be a positive safe integer timestamp or null`);
	}
	return Number(result);
}

function normalizedColorField(value: Record<string, unknown>, field: string) {
	const result = stringField(value, field);
	if (!/^#[0-9A-F]{6}$/i.test(result)) {
		throw new Error(`desktop dialog field ${JSON.stringify(field)} must be a normalized #RRGGBB color`);
	}
	return result.toUpperCase();
}

function booleanField(value: Record<string, unknown>, field: string) {
	const result = value[field];
	if (typeof result !== "boolean") {
		throw new Error(`desktop dialog field ${JSON.stringify(field)} must be boolean`);
	}
	return result;
}

function nullableNumberField(value: Record<string, unknown>, field: string) {
	const result = value[field];
	if (result === null) return null;
	if (typeof result !== "number" || !Number.isFinite(result)) {
		throw new Error(`desktop dialog field ${JSON.stringify(field)} must be a finite number or null`);
	}
	return result;
}

function numberField(value: Record<string, unknown>, field: string) {
	const result = value[field];
	if (typeof result !== "number" || !Number.isFinite(result)) {
		throw new Error(`desktop dialog field ${JSON.stringify(field)} must be a finite number`);
	}
	return result;
}

function enumField<const Value extends string>(
	value: Record<string, unknown>,
	field: string,
	allowed: ReadonlySet<Value>,
): Value {
	const result = stringField(value, field);
	if (!allowed.has(result as Value)) {
		throw new Error(`desktop dialog field ${JSON.stringify(field)} has unsupported value ${JSON.stringify(result)}`);
	}
	return result as Value;
}

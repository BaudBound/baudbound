import type { Node } from "@xyflow/react";
import type { JsonValue, ScriptNodeData } from "@/lib/types";

type SelectOption = {
	label: string;
	value: string;
};

export type SerialDeviceConfig = {
	autoReconnect: boolean;
	baudRate: number;
	dataBits: number;
	deviceId: string;
	flowControl: string;
	label: string;
	parity: string;
	port: string;
	productId: string;
	stopBits: number;
	validateUsbIdentity: boolean;
	vendorId: string;
};

export const serialBaudRateOptions: SelectOption[] = [9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600].map(
	(rate) => ({
		label: String(rate),
		value: String(rate),
	}),
);

export const serialLineEndingOptions: SelectOption[] = [
	{ label: "None", value: "none" },
	{ label: "LF", value: "lf" },
	{ label: "CRLF", value: "crlf" },
];

export const serialDataBitsOptions: SelectOption[] = [5, 6, 7, 8].map((bits) => ({
	label: String(bits),
	value: String(bits),
}));

export const serialStopBitsOptions: SelectOption[] = [
	{ label: "1", value: "1" },
	{ label: "1.5", value: "1.5" },
	{ label: "2", value: "2" },
];

export const serialParityOptions: SelectOption[] = [
	{ label: "None", value: "none" },
	{ label: "Even", value: "even" },
	{ label: "Odd", value: "odd" },
	{ label: "Mark", value: "mark" },
	{ label: "Space", value: "space" },
];

export const serialFlowControlOptions: SelectOption[] = [
	{ label: "None", value: "none" },
	{ label: "Hardware RTS/CTS", value: "hardware" },
	{ label: "Software XON/XOFF", value: "software" },
];

export const serialReadModeOptions: SelectOption[] = [
	{ label: "Line", value: "line" },
	{ label: "Raw chunk", value: "raw" },
];

export function createSerialDeviceOptions(nodes: Node<ScriptNodeData>[]): SelectOption[] {
	const devices = createSerialDeviceConfigs(nodes);
	return devices.map((device) => ({
		label: `${device.label} - ${device.port} (${device.deviceId})`,
		value: device.deviceId,
	}));
}

export function createSerialDeviceConfigs(nodes: Node<ScriptNodeData>[]): SerialDeviceConfig[] {
	const devices = new Map<string, SerialDeviceConfig>();

	for (const node of nodes) {
		if (node.data.actionType !== "trigger.serial_input") {
			continue;
		}

		const deviceId = normalizeSerialDeviceId(configString(node.data.config.deviceId)) || node.id;
		if (devices.has(deviceId)) {
			continue;
		}

		devices.set(deviceId, {
			autoReconnect: node.data.config.autoReconnect !== false,
			baudRate: normalizeBaudRate(configString(node.data.config.baudRate)),
			dataBits: normalizeDataBits(configString(node.data.config.dataBits)),
			deviceId,
			flowControl: normalizeFlowControl(configString(node.data.config.flowControl)),
			label: configString(node.data.config.label).trim() || "Serial device",
			parity: normalizeParity(configString(node.data.config.parity)),
			port: configString(node.data.config.port).trim(),
			productId: normalizeUsbHexId(configString(node.data.config.productId)),
			stopBits: normalizeStopBits(configString(node.data.config.stopBits)),
			validateUsbIdentity: node.data.config.validateUsbIdentity === true,
			vendorId: normalizeUsbHexId(configString(node.data.config.vendorId)),
		});
	}

	return [...devices.values()].sort((a, b) => a.deviceId.localeCompare(b.deviceId));
}

export function normalizeSerialDeviceId(value: string) {
	return value
		.trim()
		.toLowerCase()
		.replace(/[^a-z0-9_-]+/g, "-")
		.replace(/^-+|-+$/g, "");
}

export function normalizeBaudRate(value: string) {
	const baudRate = Number(value);
	return Number.isFinite(baudRate) && baudRate > 0 ? Math.round(baudRate) : 115200;
}

function normalizeDataBits(value: string) {
	const dataBits = Number(value);
	return [5, 6, 7, 8].includes(dataBits) ? dataBits : 8;
}

function normalizeStopBits(value: string) {
	const stopBits = Number(value);
	return [1, 1.5, 2].includes(stopBits) ? stopBits : 1;
}

function normalizeParity(value: string) {
	return ["none", "even", "odd", "mark", "space"].includes(value) ? value : "none";
}

function normalizeFlowControl(value: string) {
	return ["none", "hardware", "software"].includes(value) ? value : "none";
}

function normalizeUsbHexId(value: string) {
	const trimmed = value.trim().replace(/^0x/i, "");
	return /^[0-9a-fA-F]{1,4}$/.test(trimmed) ? trimmed.toUpperCase().padStart(4, "0") : "";
}

function configString(value: JsonValue | undefined) {
	if (typeof value === "string") {
		return value;
	}

	if (value === undefined || value === null) {
		return "";
	}

	return String(value);
}

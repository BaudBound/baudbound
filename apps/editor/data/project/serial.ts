import type { Node } from "@xyflow/react";
import type { JsonValue, ScriptNodeData } from "@/lib/types";

export type SelectOption = {
	label: string;
	value: string;
};

export type SerialDeviceConfig = {
	deviceId: string;
};

export const serialLineEndingOptions: SelectOption[] = [
	{ label: "None", value: "none" },
	{ label: "LF", value: "lf" },
	{ label: "CRLF", value: "crlf" },
];

export function createSerialDeviceOptions(nodes: Node<ScriptNodeData>[]): SelectOption[] {
	const devices = createSerialDeviceConfigs(nodes);
	return devices.map((device) => ({
		label: device.deviceId,
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
			deviceId,
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

function configString(value: JsonValue | undefined) {
	if (typeof value === "string") {
		return value;
	}

	if (value === undefined || value === null) {
		return "";
	}

	return String(value);
}

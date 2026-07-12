import { Usb } from "lucide-react";
import { normalizeSerialDeviceId } from "@/data/project/serial";
import { defineNode } from "../../node-definition";
import { configString, requiredConfig } from "../validators";

export const serialInputTriggerNode = defineNode({
	actionType: "trigger.serial_input",
	capabilities: ["trigger.serial_input"],
	configFields: [{ key: "deviceId", label: "Device id", type: "text" }],
	defaultConfig: () => ({
		deviceId: "serial-device",
	}),
	description: "Start when a serial device outputs data.",
	group: "triggers",
	icon: Usb,
	kind: "trigger",
	label: "Serial Input",
	permission: { name: "serial_input", risk: "high" },
	risk: "high",
	runtimeOutputs: [
		{
			name: "device_id",
			type: "string",
			description: "Logical serial device id that produced the input.",
			example: "n-mr3zyt6f-5.device_id",
		},
		{
			name: "data",
			type: "string",
			description: "Serial data received from the device.",
			example: "n-mr3zyt6f-5.data",
		},
		{ name: "bytes", type: "number", description: "Number of bytes received.", example: "n-mr3zyt6f-5.bytes" },
		{
			name: "timestamp",
			type: "string",
			description: "Runner timestamp when serial data was received.",
			example: "n-mr3zyt6f-5.timestamp",
		},
	],
	runnerType: "serial_input",
	validateConfig: (config) => {
		const deviceId = configString(config, "deviceId").trim();
		const normalizedDeviceId = normalizeSerialDeviceId(deviceId);
		return [
			requiredConfig(config, "deviceId", "serial device id"),
			deviceId && deviceId !== normalizedDeviceId
				? "serial device id must use lowercase letters, numbers, underscores, or hyphens."
				: "",
		].filter(Boolean);
	},
	validateGraph: ({ context, node }) => {
		const deviceId = configString(node.data.config, "deviceId").trim();
		const duplicate = context.nodes.some(
			(otherNode) =>
				otherNode.id !== node.id &&
				otherNode.data.actionType === "trigger.serial_input" &&
				configString(otherNode.data.config, "deviceId").trim() === deviceId,
		);
		return duplicate && deviceId ? [`${node.id} serial device id "${deviceId}" is used by more than one trigger.`] : [];
	},
	simulation: {
		createOutput: ({ api, context, node }) => {
			const data = context.triggerPayload.data || "simulation serial input";
			return {
				failed: false,
				outputData: {
					device_id: api.getConfigString(node, "deviceId") || node.id,
					data,
					bytes: new TextEncoder().encode(data).length,
					timestamp: new Date().toISOString(),
				},
			};
		},
	},
});

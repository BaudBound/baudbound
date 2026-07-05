import { Usb } from "lucide-react";
import { normalizeSerialDeviceId } from "@/data/project/serial";
import { defineNode } from "../../node-definition";
import {
	serialBaudRateOptions,
	serialDataBitsOptions,
	serialFlowControlOptions,
	serialParityOptions,
	serialReadModeOptions,
	serialStopBitsOptions,
} from "../options";
import { triggerPorts } from "../shared";
import { configString, requiredConfig } from "../validators";

export const serialInputTriggerNode = defineNode({
	actionType: "trigger.serial_input",
	capabilities: ["trigger.serial_input"],
	configFields: [
		{ key: "deviceId", label: "Device id", type: "text" },
		{ key: "label", label: "Label", type: "text" },
		{
			key: "port",
			label: "Port",
			type: "text",
			help: "Runner-side serial port name or path, for example COM3, /dev/ttyUSB0, or /dev/cu.usbserial-0001.",
		},
		{ key: "baudRate", label: "Baud rate", type: "select", options: serialBaudRateOptions },
		{ key: "dataBits", label: "Data bits", type: "select", options: serialDataBitsOptions },
		{ key: "parity", label: "Parity", type: "select", options: serialParityOptions },
		{ key: "stopBits", label: "Stop bits", type: "select", options: serialStopBitsOptions },
		{ key: "flowControl", label: "Flow control", type: "select", options: serialFlowControlOptions },
		{ key: "readMode", label: "Read mode", type: "select", options: serialReadModeOptions },
	],
	defaultConfig: () => ({
		deviceId: "serial-device",
		label: "Serial device",
		port: "COM3",
		baudRate: "115200",
		dataBits: "8",
		parity: "none",
		stopBits: "1",
		flowControl: "none",
		readMode: "line",
	}),
	description: "Start when a serial device outputs data.",
	group: "triggers",
	icon: Usb,
	kind: "trigger",
	label: "Serial Input",
	ports: triggerPorts,
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
			requiredConfig(config, "port", "runner serial port such as COM3 or /dev/ttyUSB0"),
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

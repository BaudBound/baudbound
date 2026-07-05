import { Usb } from "lucide-react";
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

export const serialInputTriggerNode = defineNode({
	actionType: "trigger.serial_input",
	capabilities: ["trigger.serial"],
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
	risk: "medium",
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

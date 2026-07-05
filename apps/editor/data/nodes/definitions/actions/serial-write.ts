import { Usb } from "lucide-react";
import { defineNode } from "../../node-definition";

export const serialWriteNode = defineNode({
	actionType: "action.serial.write",
	capabilities: ["action.serial"],
	configFields: [],
	defaultConfig: () => ({ deviceId: "serial-device", data: "ping", lineEnding: "none" }),
	description: "Write data to a configured serial device.",
	fallible: true,
	group: "actions",
	icon: Usb,
	kind: "action",
	label: "Serial Write",
	permission: { name: "serial_write", risk: "medium" },
	risk: "medium",
	runnerType: "serial_write",
	simulation: {
		createOutput: () => ({ failed: false, outputData: {} }),
		describe: ({ api, context, node }) => [
			{
				level: "info",
				message: `[Simulation] Serial Write (${node.id}) succeeded. Would write ${api.formatValue(api.resolveTemplate(api.getConfigString(node, "data"), context))} to serial device ${api.getConfigString(node, "deviceId")} ${getSerialLineEndingDetail(api.getConfigString(node, "lineEnding"))}.`,
			},
		],
	},
});

function getSerialLineEndingDetail(lineEnding: string) {
	if (lineEnding === "crlf") {
		return "with CRLF line ending";
	}

	if (lineEnding === "lf") {
		return "with LF line ending";
	}

	return "without an added line ending";
}

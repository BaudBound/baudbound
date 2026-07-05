import { Hash } from "lucide-react";
import { defaultInputPort, defineNode } from "../../node-definition";
import { createSwitchCaseRow, createSwitchOutputPorts, getSwitchCaseRowsFromValue } from "../rows";

export const switchNode = defineNode({
	actionType: "control.switch",
	capabilities: ["runtime.switch"],
	controlType: "switch",
	defaultConfig: () => ({ value: "{{status}}", cases: [createSwitchCaseRow("ok"), createSwitchCaseRow("warning")] }),
	description: "Match one value against cases.",
	group: "control",
	icon: Hash,
	kind: "control",
	label: "Switch",
	ports: (config) => ({
		inputs: [defaultInputPort],
		outputs: createSwitchOutputPorts(getSwitchCaseRowsFromValue(config?.cases)),
	}),
	risk: "low",
});

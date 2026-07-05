import { GitBranch } from "lucide-react";
import { defaultInputPort, defineNode } from "../../node-definition";
import { createConditionRow } from "../rows";

export const ifElseNode = defineNode({
	actionType: "control.if",
	capabilities: ["runtime.if"],
	controlType: "if",
	defaultConfig: () => ({ conditions: [createConditionRow("{{n-mr3zyt6f-12.status_code}}", "200")] }),
	description: "Branch execution by condition.",
	group: "control",
	icon: GitBranch,
	kind: "control",
	label: "If / Else",
	ports: () => ({
		inputs: [defaultInputPort],
		outputs: [
			{ id: "true", label: "true" },
			{ id: "false", label: "false" },
		],
	}),
	risk: "low",
});

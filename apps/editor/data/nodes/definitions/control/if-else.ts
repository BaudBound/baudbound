import { GitBranch } from "lucide-react";
import { defineNode } from "../../node-definition";
import { createConditionRow } from "../rows";
import { validateConditionRowsConfig } from "../shared";

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
	portPolicy: { kind: "fixed", inputs: ["input"], outputs: ["true", "false"] },
	risk: "low",
	validateConfig: (config) => validateConditionRowsConfig(config, "if/else", true),
});

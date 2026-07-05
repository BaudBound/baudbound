import type { RiskLevel, ScriptNodeData } from "@/lib/types";

export const riskTone: Record<RiskLevel, { backgroundColor: string; borderColor: string; color: string }> = {
	low: {
		color: "#3ecf8e",
		backgroundColor: "rgb(62 207 142 / 0.1)",
		borderColor: "rgb(62 207 142 / 0.32)",
	},
	medium: {
		color: "#f5a623",
		backgroundColor: "rgb(245 166 35 / 0.1)",
		borderColor: "rgb(245 166 35 / 0.32)",
	},
	high: {
		color: "#e05c5c",
		backgroundColor: "rgb(224 92 92 / 0.1)",
		borderColor: "rgb(224 92 92 / 0.32)",
	},
	dangerous: {
		color: "#a78bfa",
		backgroundColor: "rgb(167 139 250 / 0.1)",
		borderColor: "rgb(167 139 250 / 0.32)",
	},
};

export const kindAccentClassName = {
	trigger: "bg-baud-blue",
	control: "bg-baud-purple",
	action: "bg-baud-red",
} satisfies Record<ScriptNodeData["kind"], string>;

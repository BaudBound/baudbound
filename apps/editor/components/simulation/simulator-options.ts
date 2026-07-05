import type { SimulationOverrideOutcome, SimulationSpeed } from "@/lib/types";

export const outcomeOptions: Array<{ label: string; value: SimulationOverrideOutcome }> = [
	{ label: "Success", value: "success" },
	{ label: "Failed", value: "failed" },
];

export const speedOptions: Array<{ label: string; value: SimulationSpeed }> = [
	{ label: "Slow", value: "slow" },
	{ label: "Normal", value: "normal" },
	{ label: "Fast", value: "fast" },
	{ label: "Instant", value: "instant" },
];

export function normalizeSimulationSpeed(value: string): SimulationSpeed {
	return value === "slow" || value === "fast" || value === "instant" ? value : "normal";
}

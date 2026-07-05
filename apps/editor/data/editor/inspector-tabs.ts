import type { InspectorTab } from "@/lib/types";

export const inspectorTabs: Array<{ id: InspectorTab; label: string; shortLabel: string }> = [
	{ id: "properties", label: "Properties", shortLabel: "Props" },
	{ id: "simulator", label: "Simulator", shortLabel: "Sim" },
];

import type { Edge, Node } from "@xyflow/react";
import type {
	EditorAsset,
	JsonValue,
	LogEntry,
	ScriptNodeData,
	SimulationOverride,
	SimulationTraceEntry,
	SimulationTriggerPayload,
	SimulationVariableSnapshot,
} from "@/lib/types";

export type SimulationRunOptions = {
	assets: EditorAsset[];
	edges: Edge[];
	nodes: Node<ScriptNodeData>[];
	onStep?: (
		step: SimulationStep,
	) => Promise<SimulationSideEffectResult[] | undefined> | SimulationSideEffectResult[] | undefined;
	overrides: SimulationOverride[];
	signal?: AbortSignal;
	stepDelayMs?: number;
	triggerNodeId?: string;
	triggerPayload?: SimulationTriggerPayload;
};

export type SimulationStep = {
	outputLogs: LogEntry[];
	sideEffects: SimulationSideEffect[];
	traces: SimulationTraceEntry[];
	variables: SimulationVariableSnapshot[];
};

export type SimulationSideEffect = {
	nodeId: string;
} & (
	| {
			assetPath: string;
			type: "play_audio_asset";
	  }
	| {
			message: string;
			title: string;
			type: "notification_toast";
	  }
	| {
			buttons: string[];
			message: string;
			title: string;
			type: "message_box";
			variant: "error" | "info" | "question" | "warning";
	  }
	| {
			durationMs: number;
			frequencyHz: number;
			type: "system_beep";
	  }
);

export type SimulationSideEffectResult = {
	button: string;
	nodeId: string;
	type: "message_box";
};

export type SimulationRun = {
	steps: SimulationStep[];
	status: "completed" | "failed";
};

export type SimulationContext = {
	assetsByPackagePath: Map<string, EditorAsset>;
	edgesBySource: Map<string, Edge[]>;
	failed: boolean;
	halted: boolean;
	nodeOutputs: Record<string, Record<string, JsonValue>>;
	nodesById: Map<string, Node<ScriptNodeData>>;
	onStep?: (
		step: SimulationStep,
	) => Promise<SimulationSideEffectResult[] | undefined> | SimulationSideEffectResult[] | undefined;
	overridesByNodeId: Map<string, SimulationOverride["outcome"]>;
	runtimeVariables: Record<string, JsonValue>;
	signal?: AbortSignal;
	stepDelayMs: number;
	streamedSteps: number;
	steps: SimulationStep[];
	triggerPayload: SimulationTriggerPayload;
	visitedEdges: Map<string, number>;
};

export type NodeExecutionResult = {
	failed: boolean;
	outputData: Record<string, JsonValue>;
};

import type { Edge, Node } from "@xyflow/react";
import type {
	EditorAsset,
	JsonValue,
	LogEntry,
	ProjectSettings,
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
	projectSettings: ProjectSettings;
	secretValues?: Record<string, JsonValue>;
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
			variant: "error" | "info" | "warning";
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
	finalVariables: SimulationVariableSnapshot[];
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
	secretNames: Set<string>;
	secretValues: JsonValue[];
	signal?: AbortSignal;
	stepDelayMs: number;
	streamedSteps: number;
	triggerPayload: SimulationTriggerPayload;
	webhookResponse: {
		fallback: Record<string, JsonValue>;
		response?: Record<string, JsonValue>;
		sent: boolean;
		triggerNodeId: string;
		waiting: boolean;
	} | null;
};

export type NodeExecutionResult = {
	failed: boolean;
	outputData: Record<string, JsonValue>;
};

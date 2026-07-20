import type { Edge, Node } from "@xyflow/react";
import type { LucideIcon } from "lucide-react";
import type {
	ActionType,
	CapabilitySummary,
	EditorAsset,
	JsonValue,
	LogEntry,
	NodeKind,
	NodePort,
	PermissionSummary,
	RiskLevel,
	RuntimeDataField,
	RuntimeDataOutput,
	ScriptNodeData,
	SimulationTraceEntry,
	TargetRuntime,
} from "@/lib/types";
import type {
	NodeExecutionResult,
	SimulationContext,
	SimulationSideEffect,
	SimulationSideEffectResult,
} from "@/utils/simulation-types";
import type { SelectOption } from "./definitions/options";

export type NumericConfigContract = {
	kind: "float" | "integer";
	signed: boolean;
	minimum: string;
	maximum: string;
	minimumInclusive: boolean;
	maximumInclusive: boolean;
};

export type NumericConfigCondition = {
	key: string;
	equals: string;
};

type NodeConfigFieldBase = {
	colorPicker?: boolean;
	key: string;
	label: string;
	options?: SelectOption[];
	required?: boolean;
	usesVariables?: boolean;
	help?: string;
	visibleWhen?: NumericConfigCondition;
};

export type NodeConfigField = NodeConfigFieldBase &
	(
		| { numeric: NumericConfigContract; numericWhen?: never; type: "number" }
		| { numeric: NumericConfigContract; numericWhen: NumericConfigCondition; type: "text" }
		| { numeric?: never; numericWhen?: never; type: "select" | "switch" | "text" | "textarea" }
	);

export type NodeDefinitionGroupId = "triggers" | "control" | "actions";

export type NodePorts = {
	inputs: NodePort[];
	outputs: NodePort[];
};

export type NodePortPolicy =
	| { inputs: string[]; kind: "fixed"; outputs: string[] }
	| { configKey: string; kind: "switch-cases"; outputPrefix: string };

export type NodePermissionPathRule = {
	access: "read" | "write";
	configKey: string;
};

export type NodeSimulationApi = {
	createError: (
		message: string,
		code: string,
		type: string,
		details?: Record<string, JsonValue>,
		retryable?: boolean,
	) => Record<string, JsonValue>;
	createPixelColorOutput: (x: number, y: number) => Record<string, JsonValue>;
	executeHttpRequest: (node: Node<ScriptNodeData>, context: SimulationContext) => Promise<NodeExecutionResult>;
	formatValue: (value: JsonValue) => string;
	getConfigString: (node: Node<ScriptNodeData>, key: string) => string;
	parseJsonValue: (value: string) => JsonValue | undefined;
	resolveJsonCompatibleInput: (value: string, context: SimulationContext) => JsonValue;
	resolveTemplate: (value: string, context: SimulationContext) => JsonValue;
	validatePlaySound: (node: Node<ScriptNodeData>, context: SimulationContext) => NodeExecutionResult;
};

export type NodeSimulationDefinition = {
	afterExecute?: (params: {
		api: NodeSimulationApi;
		context: SimulationContext;
		failed: boolean;
		node: Node<ScriptNodeData>;
		sideEffectResults: SimulationSideEffectResult[];
	}) => Promise<SimulationTraceEntry[] | undefined> | SimulationTraceEntry[] | undefined;
	createOutput?: (params: {
		api: NodeSimulationApi;
		context: SimulationContext;
		forcedFailed: boolean;
		node: Node<ScriptNodeData>;
	}) => NodeExecutionResult | Promise<NodeExecutionResult>;
	describe?: (params: {
		api: NodeSimulationApi;
		context: SimulationContext;
		failed: boolean;
		node: Node<ScriptNodeData>;
		override: "success" | "failed" | undefined;
	}) => SimulationTraceEntry[];
	outputLogs?: (params: {
		api: NodeSimulationApi;
		context: SimulationContext;
		failed: boolean;
		node: Node<ScriptNodeData>;
	}) => LogEntry[];
	sideEffects?: (params: {
		api: NodeSimulationApi;
		context: SimulationContext;
		node: Node<ScriptNodeData>;
	}) => SimulationSideEffect[];
};

export type NodeGraphValidationContext = {
	assets: EditorAsset[];
	edges: Edge[];
	nodes: Node<ScriptNodeData>[];
};

export type NodeDefinition = {
	actionType: ActionType;
	capabilities: CapabilitySummary["name"][];
	configFields?: NodeConfigField[];
	controlType?: string;
	defaultConfig?: () => Record<string, JsonValue>;
	description: string;
	desktopOnly?: boolean;
	fallible?: boolean;
	group: NodeDefinitionGroupId;
	icon: LucideIcon;
	kind: NodeKind;
	label: string;
	permission?: PermissionSummary;
	permissionPathRules?: readonly NodePermissionPathRule[];
	portPolicy?: NodePortPolicy;
	deriveCapabilities?: (config: Record<string, JsonValue>) => CapabilitySummary["name"][];
	derivePermissions?: (config: Record<string, JsonValue>) => PermissionSummary[];
	risk: RiskLevel;
	runtimeOutputs?: RuntimeDataOutput[];
	runnerType?: string;
	sanitizeConfig?: (config: Record<string, JsonValue>) => Record<string, JsonValue>;
	simulation?: NodeSimulationDefinition;
	supportedTargetRuntimes?: readonly TargetRuntime[];
	validateConfig?: (config: Record<string, JsonValue>) => string[];
	validateGraph?: (params: { context: NodeGraphValidationContext; node: Node<ScriptNodeData> }) => string[];
	validateTargetRuntime?: (params: { config: Record<string, JsonValue>; targetRuntime: TargetRuntime }) => string[];
};

export const defaultInputPort: NodePort = { id: "input", label: "input" };
export const defaultOutputPort: NodePort = { id: "out", label: "out" };
export const triggerOutputPort: NodePort = { id: "out", label: "out" };

export const fallibleActionOutputs: NodePort[] = [
	{ id: "success", label: "success" },
	{ id: "failed", label: "failed" },
];

export const runtimeErrorFields: RuntimeDataField[] = [
	{
		name: "message",
		type: "string",
		description: "Human-readable failure message.",
		example: "n-mr3zyt6f-12.error.message",
	},
	{
		name: "code",
		type: "string",
		description: "Stable runner error code such as TIMEOUT, PERMISSION_DENIED, or PROCESS_EXIT_FAILED.",
		example: "n-mr3zyt6f-12.error.code",
	},
	{
		name: "type",
		type: "string",
		description: "Broad error category such as network, permission, validation, runtime, process, or unknown.",
		example: "n-mr3zyt6f-12.error.type",
	},
	{
		name: "retryable",
		type: "boolean",
		description: "Whether retrying the failed operation may succeed.",
		example: "n-mr3zyt6f-12.error.retryable",
	},
	{
		name: "details",
		type: "object",
		description: "Action-specific structured details. Empty object when no extra details are available.",
		example: "n-mr3zyt6f-12.error.details",
	},
];

export const failureErrorOutput: RuntimeDataOutput = {
	name: "error",
	type: "object",
	description: "Structured failure details when the node continues through the failed output.",
	example: "n-mr3zyt6f-12.error.message",
	fields: runtimeErrorFields,
};

export function withFailureErrorOutput(outputs: RuntimeDataOutput[]) {
	return [...outputs, failureErrorOutput];
}

export function defineNode<const Definition extends NodeDefinition>(definition: Definition) {
	return definition;
}

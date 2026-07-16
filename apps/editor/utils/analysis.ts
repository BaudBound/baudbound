import type { Edge, Node } from "@xyflow/react";
import {
	getControlStepType,
	getNodeCapabilities,
	getNodePermissions,
	getRunnerActionType,
	getRunnerTriggerType,
	sanitizeNodeConfig,
} from "@/data/nodes/registry";
import { createBuiltInVariableRuntimeContext } from "@/data/project/built-in-variables";
import { createNodeOutputVariables } from "@/data/project/variables";
import type {
	ActionType,
	CapabilitySummary,
	DefaultVariable,
	EditorAsset,
	ExecutableActionType,
	ExportSummary,
	LogEntry,
	PermissionSummary,
	ProjectSettings,
	RiskLevel,
	ScriptNodeData,
	SecretDeclaration,
	TargetRuntime,
	TriggerActionType,
} from "../lib/types";
import { getEdgeExecutionOrder } from "./editor-graph";

const riskWeight: Record<RiskLevel, number> = {
	low: 1,
	medium: 2,
	high: 3,
	dangerous: 4,
};

export function calculatePermissions(
	nodes: Node<ScriptNodeData>[],
	secretDeclarations: SecretDeclaration[] = [],
	defaultVariables: DefaultVariable[] = [],
): PermissionSummary[] {
	const permissions = new Map<string, PermissionSummary>();

	for (const node of nodes) {
		for (const permission of getNodePermissions(node.data.actionType, node.data.config)) {
			const existing = permissions.get(permission.name);
			if (!existing || riskWeight[permission.risk] > riskWeight[existing.risk]) {
				permissions.set(permission.name, permission);
			}
		}
	}
	if (secretDeclarations.length > 0) {
		permissions.set("read_secret", { name: "read_secret", risk: "high" });
	}
	if (defaultVariables.some((variable) => variable.scope === "runtime")) {
		permissions.set("set_local_variable", { name: "set_local_variable", risk: "low" });
	}
	if (defaultVariables.some((variable) => variable.scope === "persistent")) {
		permissions.set("set_persistent_variable", { name: "set_persistent_variable", risk: "medium" });
	}

	return [...permissions.values()].sort(
		(a, b) => riskWeight[a.risk] - riskWeight[b.risk] || a.name.localeCompare(b.name),
	);
}

export function calculateCapabilities(
	nodes: Node<ScriptNodeData>[],
	secretDeclarations: SecretDeclaration[] = [],
	defaultVariables: DefaultVariable[] = [],
): CapabilitySummary[] {
	const capabilities = new Set<string>();

	for (const node of nodes) {
		for (const capability of getNodeCapabilities(node.data.actionType, node.data.config)) {
			capabilities.add(capability);
		}
	}
	if (secretDeclarations.length > 0) {
		capabilities.add("runtime.secrets");
	}
	if (defaultVariables.length > 0) {
		capabilities.add("runtime.variables");
	}
	if (defaultVariables.some((variable) => variable.scope === "persistent")) {
		capabilities.add("runtime.persistent_storage");
	}

	return [...capabilities].sort().map((name) => ({ name }));
}

export function calculateRiskLevel(permissions: PermissionSummary[]): RiskLevel {
	return permissions.reduce<RiskLevel>((highest, permission) => {
		return riskWeight[permission.risk] > riskWeight[highest] ? permission.risk : highest;
	}, "low");
}

export function createExportSummary(
	projectName: string,
	targetRuntime: TargetRuntime,
	minimumRunnerVersion: string,
	assets: EditorAsset[] = [],
): ExportSummary {
	return {
		filename: `${slugFromName(projectName)}.bbs`,
		formatVersion: 1,
		languageVersion: 1,
		minimumRunnerVersion,
		targetRuntime,
		contents: [
			"manifest.json",
			"program.json",
			"editor.json",
			"permissions.json",
			"capabilities.json",
			"README.md",
			...assets.map((asset) => asset.packagePath).sort(),
		],
	};
}

function slugFromName(name: string) {
	return (
		name
			.trim()
			.toLowerCase()
			.replace(/[^a-z0-9]+/g, "-")
			.replace(/^-+|-+$/g, "") || "untitled-script"
	);
}

export function createConsoleLogs(
	scriptName: string,
	targetRuntime: TargetRuntime,
	permissions: PermissionSummary[],
): LogEntry[] {
	const risk = calculateRiskLevel(permissions);
	const logs: LogEntry[] = [
		{ level: "info", message: `Script loaded: ${scriptName}` },
		{ level: "info", message: `Target runtime: ${targetRuntime}` },
		{ level: "info", message: `Permissions calculated: ${permissions.length} required` },
	];

	if (risk !== "low") {
		logs.push({ level: "warn", message: `${risk} risk action requires approval on import` });
	}

	logs.push({ level: "info", message: "Script is valid. Ready to export." });

	return logs;
}

export function toProgramJson(nodes: Node<ScriptNodeData>[], edges: Edge[], projectSettings: ProjectSettings) {
	const triggers = nodes.filter((node) => node.data.kind === "trigger").map(toTriggerJson);
	const steps = nodes.filter((node) => node.data.kind !== "trigger").map(toStepJson);
	const builtInVariableContext = createBuiltInVariableRuntimeContext(projectSettings);
	const nodeOutputVariables = createNodeOutputVariables(nodes);

	if (triggers.length === 0) {
		throw new Error("Cannot export a script without at least one trigger node.");
	}

	return {
		entry: {
			trigger: triggers[0],
			triggers,
			program: {
				type: "block",
				execution_model: "directed_graph",
				runtime_context: {
					expression_reference: "{{node-id.data_name}}",
					template_reference: "{{node-id.data_name}}",
					variables: [...builtInVariableContext.variables, ...nodeOutputVariables],
					built_in_variables: builtInVariableContext,
					node_outputs: nodes
						.filter((node) => (node.data.runtimeOutputs ?? []).length > 0)
						.map((node) => ({
							id: node.id,
							action_type: node.data.actionType,
							outputs: node.data.runtimeOutputs ?? [],
						})),
				},
				steps,
				edges: edges.map((edge) => ({
					execution_order: requireEdgeExecutionOrder(edge),
					source: edge.source,
					source_handle: edge.sourceHandle,
					target: edge.target,
					target_handle: edge.targetHandle,
				})),
			},
		},
	};
}

function requireEdgeExecutionOrder(edge: Edge) {
	const executionOrder = getEdgeExecutionOrder(edge);
	if (executionOrder === null) {
		throw new Error(`Connection ${edge.id} is missing its execution order.`);
	}
	return executionOrder;
}

function toTriggerJson(node: Node<ScriptNodeData>) {
	const actionType = node.data.actionType;

	if (!isTriggerActionType(actionType)) {
		throw new Error(`Node ${node.id} is marked as trigger but uses non-trigger action type ${actionType}`);
	}

	return {
		id: node.id,
		action_type: actionType,
		type: getRunnerTriggerType(actionType),
		config: sanitizeNodeConfig(actionType, node.data.config),
		runtime_outputs: node.data.runtimeOutputs ?? [],
	};
}

function toStepJson(node: Node<ScriptNodeData>) {
	const base = {
		id: node.id,
		action_type: node.data.actionType,
		config: sanitizeNodeConfig(node.data.actionType, node.data.config),
		runtime_outputs: node.data.runtimeOutputs ?? [],
	};

	if (node.data.kind === "control") {
		return {
			...base,
			type: getControlStepType(node.data.actionType),
		};
	}

	if (node.data.actionType === "runtime.set_variable") {
		return {
			...base,
			type: "set_variable",
		};
	}

	if (!isExecutableActionType(node.data.actionType)) {
		throw new Error(`Unsupported action type in export: ${node.data.actionType}`);
	}

	return {
		...base,
		type: "action",
		action: getRunnerActionType(node.data.actionType),
	};
}

function isTriggerActionType(actionType: ActionType): actionType is TriggerActionType {
	return actionType.startsWith("trigger.");
}

function isExecutableActionType(actionType: ActionType): actionType is ExecutableActionType {
	return actionType.startsWith("action.");
}

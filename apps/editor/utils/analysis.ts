import type { Edge, Node } from "@xyflow/react";
import {
	getControlStepType,
	getNodeCapabilities,
	getNodePermission,
	getRunnerActionType,
	getRunnerTriggerType,
	sanitizeNodeConfig,
} from "@/data/nodes/registry";
import { createBuiltInVariableRuntimeContext } from "@/data/project/built-in-variables";
import { createNodeOutputVariables } from "@/data/project/variables";
import type {
	ActionType,
	CapabilitySummary,
	EditorAsset,
	ExecutableActionType,
	ExportSummary,
	JsonValue,
	LogEntry,
	PermissionSummary,
	ProjectSettings,
	RiskLevel,
	ScriptNodeData,
	TargetRuntime,
	TriggerActionType,
} from "../lib/types";

const riskWeight: Record<RiskLevel, number> = {
	low: 1,
	medium: 2,
	high: 3,
	dangerous: 4,
};

export function calculatePermissions(nodes: Node<ScriptNodeData>[]): PermissionSummary[] {
	const permissions = new Map<string, PermissionSummary>();

	for (const node of nodes) {
		const permission = getNodePermission(node.data.actionType);
		if (!permission) {
			continue;
		}

		const existing = permissions.get(permission.name);
		if (!existing || riskWeight[permission.risk] > riskWeight[existing.risk]) {
			permissions.set(permission.name, permission);
		}
	}

	return [...permissions.values()].sort(
		(a, b) => riskWeight[a.risk] - riskWeight[b.risk] || a.name.localeCompare(b.name),
	);
}

export function calculateCapabilities(nodes: Node<ScriptNodeData>[]): CapabilitySummary[] {
	const capabilities = new Set<string>();

	for (const node of nodes) {
		for (const capability of getNodeCapabilities(node.data.actionType)) {
			capabilities.add(capability);
		}
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
	assets: EditorAsset[] = [],
): ExportSummary {
	return {
		filename: `${slugFromName(projectName)}.bbs`,
		formatVersion: 1,
		languageVersion: 1,
		minimumRunnerVersion: "0.1.0",
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

	return {
		entry: {
			trigger: triggers[0] ?? createManualTriggerFallback(),
			triggers,
			program: {
				type: "block",
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
					source: edge.source,
					source_handle: edge.sourceHandle,
					target: edge.target,
					target_handle: edge.targetHandle,
				})),
			},
		},
	};
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
		config: node.data.config,
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

function createManualTriggerFallback(): {
	id: string;
	action_type: "trigger.manual";
	type: "manual";
	config: Record<string, JsonValue>;
	runtime_outputs: [];
} {
	return {
		id: "implicit-manual-trigger",
		action_type: "trigger.manual",
		type: "manual",
		config: {},
		runtime_outputs: [],
	};
}

function isTriggerActionType(actionType: ActionType): actionType is TriggerActionType {
	return actionType.startsWith("trigger.");
}

function isExecutableActionType(actionType: ActionType): actionType is ExecutableActionType {
	return actionType.startsWith("action.");
}

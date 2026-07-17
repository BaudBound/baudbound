import { isEditorEdgeStyle } from "@/data/editor/flow-canvas";
import {
	getControlStepType,
	getNodeCapabilities,
	getNodeDefinition,
	getNodePermissions,
	getNodePorts,
	getRunnerActionType,
	getRunnerTriggerType,
	getTargetRuntimeCompatibilityErrors,
	validateNodeConfig,
} from "@/data/nodes/registry";
import { targetRuntimes } from "@/data/project/runtimes";
import { variableTypes } from "@/data/project/variables";
import type { ActionType, JsonValue, PermissionSummary, RiskLevel, TargetRuntime } from "@/lib/types";
import { isSelfConnection } from "@/utils/editor-graph";

export const canonicalCapabilities = [
	"trigger.manual",
	"trigger.schedule",
	"trigger.hotkey",
	"trigger.file_watch",
	"trigger.webhook",
	"trigger.websocket",
	"trigger.serial_input",
	"trigger.startup",
	"trigger.process_started",
	"action.log",
	"action.delay",
	"action.notification",
	"action.message_box",
	"action.http",
	"action.webhook_response",
	"action.websocket",
	"action.file",
	"action.process",
	"action.keyboard",
	"action.mouse",
	"action.clipboard",
	"action.window",
	"action.sound",
	"action.text",
	"action.calculate",
	"action.pixel",
	"action.serial",
	"action.sub_script",
	"runtime.variables",
	"runtime.color_match",
	"runtime.if",
	"runtime.switch",
	"runtime.loop",
	"runtime.while",
	"runtime.for_each",
	"runtime.sub_script",
	"runtime.error_handling",
	"runtime.persistent_storage",
	"runtime.secrets",
] as const;

export const canonicalPermissions = [
	"log",
	"delay",
	"beep",
	"math",
	"calculate",
	"text_transform",
	"set_local_variable",
	"set_persistent_variable",
	"set_global_variable",
	"read_secret",
	"read_runtime_data",
	"show_notification",
	"show_message_box",
	"http_request",
	"download_file",
	"file_read",
	"file_copy",
	"file_move",
	"write_clipboard",
	"open_application",
	"window_query",
	"process_query",
	"serial_write",
	"keyboard_control",
	"mouse_control",
	"screen_pixel_read",
	"play_sound",
	"file_write_limited",
	"run_process",
	"run_shell_command",
	"delete_file",
	"read_sensitive_file",
	"write_any_file",
	"read_clipboard",
	"startup_trigger",
	"webhook_public_bind",
	"webhook_response",
	"websocket_public_bind",
	"websocket_write",
	"serial_input",
	"window_focus",
	"process_kill",
	"sub_script_run",
] as const;

const capabilitySet = new Set<string>(canonicalCapabilities);
const permissionSet = new Set<string>(canonicalPermissions);
const riskWeight: Record<RiskLevel, number> = {
	low: 1,
	medium: 2,
	high: 3,
	dangerous: 4,
};

type PackageJsonFiles = Record<string, unknown>;

export function validatePackageJsonContracts(jsonFiles: PackageJsonFiles) {
	return [
		...validateManifestContract(jsonFiles["manifest.json"]),
		...validateDefaultVariableProgramContract(jsonFiles["manifest.json"], jsonFiles["program.json"]),
		...validateProgramContract(jsonFiles["program.json"]),
		...validatePermissionsContract(
			jsonFiles["permissions.json"],
			jsonFiles["program.json"],
			jsonFiles["manifest.json"],
		),
		...validateCapabilitiesContract(
			jsonFiles["capabilities.json"],
			jsonFiles["program.json"],
			jsonFiles["manifest.json"],
		),
		...validateEditorContract(jsonFiles["editor.json"]),
	];
}

export function validateManifestContract(value: unknown) {
	const errors: string[] = [];
	const manifest = asRecord(value);
	if (!manifest) {
		return ["manifest.json must be an object."];
	}

	for (const field of [
		"format_version",
		"script_language_version",
		"id",
		"name",
		"created_with",
		"created_at",
		"minimum_runner_version",
	]) {
		if (manifest[field] === undefined || manifest[field] === "") {
			errors.push(`manifest.json is missing ${field}.`);
		}
	}

	if (typeof manifest.format_version !== "number" || manifest.format_version < 1) {
		errors.push("manifest.json format_version must be a positive number.");
	}
	if (typeof manifest.script_language_version !== "number" || manifest.script_language_version < 1) {
		errors.push("manifest.json script_language_version must be a positive number.");
	}
	if (typeof manifest.name !== "string" || !manifest.name.trim()) {
		errors.push("manifest.json name must be a non-empty string.");
	}
	if (manifest.tags !== undefined && !isStringArray(manifest.tags)) {
		errors.push("manifest.json tags must be an array of strings.");
	}
	if (manifest.assets !== undefined && !Array.isArray(manifest.assets)) {
		errors.push("manifest.json assets must be an array when present.");
	}
	if (manifest.secrets !== undefined) {
		if (!Array.isArray(manifest.secrets)) {
			errors.push("manifest.json secrets must be an array when present.");
		} else {
			const names = new Set<string>();
			for (const value of manifest.secrets) {
				const secret = asRecord(value);
				if (!secret) {
					errors.push("manifest.json secret declarations must be objects.");
					continue;
				}
				const name = typeof secret.name === "string" ? secret.name : "";
				if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name) || name.startsWith("system_") || name.startsWith("manifest_")) {
					errors.push(`manifest.json secret name "${name}" is invalid or reserved.`);
				}
				if (names.has(name)) {
					errors.push(`manifest.json contains duplicate secret name "${name}".`);
				}
				names.add(name);
				if (!variableTypes.includes(secret.type as (typeof variableTypes)[number])) {
					errors.push(`manifest.json secret "${name}" has invalid type "${String(secret.type)}".`);
				}
				if (typeof secret.required !== "boolean") {
					errors.push(`manifest.json secret "${name}" required must be boolean.`);
				}
				if (secret.description !== undefined && typeof secret.description !== "string") {
					errors.push(`manifest.json secret "${name}" description must be a string.`);
				}
			}
		}
	}
	if (manifest.variables !== undefined) {
		if (!Array.isArray(manifest.variables)) {
			errors.push("manifest.json variables must be an array when present.");
		} else {
			const names = new Set<string>();
			const secretNames = new Set(
				Array.isArray(manifest.secrets)
					? manifest.secrets.flatMap((value) => {
							const secret = asRecord(value);
							return typeof secret?.name === "string" ? [secret.name] : [];
						})
					: [],
			);
			for (const value of manifest.variables) {
				const variable = asRecord(value);
				if (!variable) {
					errors.push("manifest.json variable declarations must be objects.");
					continue;
				}
				const name = typeof variable.name === "string" ? variable.name : "";
				const type = variable.type as (typeof variableTypes)[number];
				if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name) || name.startsWith("system_") || name.startsWith("manifest_")) {
					errors.push(`manifest.json variable name "${name}" is invalid or reserved.`);
				}
				if (names.has(name)) errors.push(`manifest.json contains duplicate variable name "${name}".`);
				if (secretNames.has(name)) errors.push(`manifest.json variable "${name}" conflicts with a secret declaration.`);
				names.add(name);
				if (variable.scope !== "runtime" && variable.scope !== "persistent") {
					errors.push(`manifest.json variable "${name}" has invalid scope "${String(variable.scope)}".`);
				}
				if (!variableTypes.includes(type)) {
					errors.push(`manifest.json variable "${name}" has invalid type "${String(variable.type)}".`);
				} else if (!defaultValueMatchesType(type, variable.value)) {
					errors.push(`manifest.json variable "${name}" value does not match type "${type}".`);
				}
				if (variable.description !== undefined && typeof variable.description !== "string") {
					errors.push(`manifest.json variable "${name}" description must be a string.`);
				}
			}
		}
	}

	return errors;
}

function validateDefaultVariableProgramContract(manifestValue: unknown, programValue: unknown) {
	const manifest = asRecord(manifestValue);
	const program = asRecord(programValue);
	const entry = asRecord(program?.entry);
	const block = asRecord(entry?.program);
	if (!Array.isArray(manifest?.variables) || !Array.isArray(block?.steps)) {
		return [];
	}

	const defaults = new Map(
		manifest.variables.flatMap((value) => {
			const variable = asRecord(value);
			return typeof variable?.name === "string" ? [[variable.name, variable] as const] : [];
		}),
	);
	return block.steps.flatMap((value) => {
		const step = asRecord(value);
		const config = asRecord(step?.config);
		if (step?.action_type !== "runtime.set_variable" || typeof config?.name !== "string") return [];
		const variable = defaults.get(config.name);
		if (!variable || (variable.scope === config.scope && variable.type === config.valueType)) return [];
		return [`manifest.json variable "${config.name}" must match its Variable Operation scope and type.`];
	});
}

function defaultValueMatchesType(type: (typeof variableTypes)[number], value: unknown) {
	if (type === "string") return typeof value === "string" && value.trim().length > 0;
	if (type === "file_path") return typeof value === "string" && value.trim().length > 0;
	if (type === "number") return typeof value === "number" && Number.isFinite(value);
	if (type === "boolean") return typeof value === "boolean";
	if (type === "list") return Array.isArray(value) && value.every(isJsonValue);
	if (type === "object") return isJsonObject(value);
	const object = asRecord(value);
	if (type === "http_response") {
		return (
			object?.type === "http_response" &&
			typeof object.status === "number" &&
			isJsonObject(object.headers) &&
			"body" in object
		);
	}
	if (type === "datetime") return object?.type === "datetime" && typeof object.value === "string";
	return object?.type === "duration" && typeof object.unit === "string" && typeof object.value === "number";
}

export function validateProgramContract(value: unknown) {
	const errors: string[] = [];
	const program = asRecord(value);
	const entry = asRecord(program?.entry);
	const block = asRecord(entry?.program);
	const triggers = Array.isArray(entry?.triggers) ? entry.triggers : [];
	const steps = Array.isArray(block?.steps) ? block.steps : [];

	if (!program || !entry || !block) {
		return ["program.json must contain entry.program."];
	}
	if (block.type !== "block") {
		errors.push('program.json entry.program.type must be "block".');
	}
	if (block.execution_model !== "directed_graph") {
		errors.push('program.json entry.program.execution_model must be "directed_graph".');
	}
	if (!Array.isArray(entry.triggers)) {
		errors.push("program.json entry.triggers must be an array.");
	}
	if (!asRecord(entry.trigger)) {
		errors.push("program.json entry.trigger must be an object.");
	} else if (
		Array.isArray(entry.triggers) &&
		typeof asRecord(entry.trigger)?.id === "string" &&
		!entry.triggers.some((trigger) => asRecord(trigger)?.id === asRecord(entry.trigger)?.id)
	) {
		errors.push("program.json entry.trigger must be present in entry.triggers.");
	}
	if (!Array.isArray(block.steps)) {
		errors.push("program.json entry.program.steps must be an array.");
	}
	if (!Array.isArray(block.edges)) {
		errors.push("program.json entry.program.edges must be an array.");
	}
	if (!asRecord(block.runtime_context)) {
		errors.push("program.json entry.program.runtime_context must be an object.");
	}

	for (const trigger of triggers) {
		errors.push(...validateProgramNode(trigger, "trigger"));
	}
	for (const step of steps) {
		errors.push(...validateProgramNode(step, "step"));
	}
	errors.push(...validateProgramGraphContract(program));

	return errors;
}

export function validateProgramGraphContract(value: unknown) {
	const errors: string[] = [];
	const program = asRecord(value);
	const entry = asRecord(program?.entry);
	const block = asRecord(entry?.program);
	const triggers = Array.isArray(entry?.triggers) ? entry.triggers : [];
	const steps = Array.isArray(block?.steps) ? block.steps : [];
	const edges = Array.isArray(block?.edges) ? block.edges : [];

	if (!program || !entry || !block) {
		return ["program.json must contain entry.program before graph validation can run."];
	}

	if (triggers.length === 0) {
		errors.push("program.json entry.triggers must contain at least one trigger.");
	}

	const manualTriggerCount = triggers.filter((trigger) => asRecord(trigger)?.action_type === "trigger.manual").length;
	if (manualTriggerCount > 1) {
		errors.push("program.json entry.triggers may contain only one manual trigger.");
	}

	const nodeRecords = [...triggers, ...steps].map((node) => asRecord(node));
	const nodeIds = new Set<string>();
	const duplicateNodeIds = new Set<string>();
	const nodesById = new Map<string, { actionType: ActionType; config: Record<string, JsonValue> }>();
	const executionOrderGroups = new Map<string, number[]>();

	for (const node of nodeRecords) {
		if (!node || typeof node.id !== "string" || !node.id.trim()) {
			errors.push("program.json graph contains a node without a non-empty id.");
			continue;
		}

		if (nodeIds.has(node.id)) {
			duplicateNodeIds.add(node.id);
		}
		nodeIds.add(node.id);

		const actionType = typeof node.action_type === "string" ? (node.action_type as ActionType) : null;
		if (!actionType || !getNodeDefinition(actionType)) {
			continue;
		}

		nodesById.set(node.id, {
			actionType,
			config: isJsonObject(node.config) ? node.config : {},
		});
	}

	for (const duplicateNodeId of duplicateNodeIds) {
		errors.push(`program.json graph contains duplicate node id "${duplicateNodeId}".`);
	}

	for (const [index, value] of edges.entries()) {
		const edge = asRecord(value);
		const label = `program.json edge ${index + 1}`;
		if (!edge) {
			errors.push(`${label} must be an object.`);
			continue;
		}

		const source = typeof edge.source === "string" ? edge.source : "";
		const target = typeof edge.target === "string" ? edge.target : "";
		if (!source || !target) {
			errors.push(`${label} must define source and target node ids.`);
			continue;
		}

		const sourceNode = nodesById.get(source);
		const targetNode = nodesById.get(target);
		if (!sourceNode) {
			errors.push(`${label} references missing source node "${source}".`);
			continue;
		}
		if (!targetNode) {
			errors.push(`${label} references missing target node "${target}".`);
			continue;
		}
		if (isSelfConnection({ source, target })) {
			errors.push(`${label} cannot connect node "${source}" to itself.`);
			continue;
		}

		const sourceHandle = typeof edge.source_handle === "string" ? edge.source_handle : "";
		const targetHandle = typeof edge.target_handle === "string" ? edge.target_handle : "";
		const executionOrder = edge.execution_order;
		const sourcePorts = getNodePorts(sourceNode.actionType, sourceNode.config);
		const targetPorts = getNodePorts(targetNode.actionType, targetNode.config);

		if (!sourceHandle) {
			errors.push(`${label} must define source_handle.`);
		} else if (!sourcePorts.outputs.some((port) => port.id === sourceHandle)) {
			errors.push(`${label} uses unknown source_handle "${sourceHandle}" on node "${source}".`);
		}

		if (typeof executionOrder !== "number" || !Number.isSafeInteger(executionOrder) || executionOrder < 0) {
			errors.push(`${label} must define a non-negative integer execution_order.`);
		} else if (sourceHandle) {
			const groupKey = `${source}\u0000${sourceHandle}`;
			const group = executionOrderGroups.get(groupKey) ?? [];
			group.push(executionOrder);
			executionOrderGroups.set(groupKey, group);
		}

		if (!targetHandle) {
			errors.push(`${label} must define target_handle.`);
		} else if (!targetPorts.inputs.some((port) => port.id === targetHandle)) {
			errors.push(`${label} uses unknown target_handle "${targetHandle}" on node "${target}".`);
		}
	}

	for (const orders of executionOrderGroups.values()) {
		orders.sort((left, right) => left - right);
		if (orders.some((order, index) => order !== index)) {
			errors.push(
				"Connections from the same source output must use unique consecutive execution_order values starting at 0.",
			);
		}
	}

	return errors;
}

export function validateCapabilitiesContract(
	capabilitiesValue: unknown,
	programValue: unknown,
	manifestValue?: unknown,
) {
	const errors: string[] = [];
	const capabilities = asRecord(capabilitiesValue);
	if (!capabilities) {
		return ["capabilities.json must be an object."];
	}

	const declared = capabilities.required_capabilities;
	if (!isStringArray(declared)) {
		errors.push("capabilities.json required_capabilities must be an array of strings.");
	} else {
		errors.push(...validateUniqueKnownStrings(declared, capabilitySet, "capability"));
		const recalculated = recalculateProgramDeclarations(programValue, manifestValue).capabilities;
		errors.push(...compareStringSets(declared, recalculated, "capabilities.json required_capabilities"));
	}

	if (
		typeof capabilities.target_runtime !== "string" ||
		!targetRuntimes.includes(capabilities.target_runtime as TargetRuntime)
	) {
		errors.push("capabilities.json target_runtime must be a supported target runtime.");
	} else {
		errors.push(
			...getTargetRuntimeCompatibilityErrors(
				getProgramCompatibilityNodes(programValue),
				capabilities.target_runtime as TargetRuntime,
			).map((error) => `capabilities.json target_runtime ${error}`),
		);
	}

	return errors;
}

export function validatePermissionsContract(permissionsValue: unknown, programValue: unknown, manifestValue?: unknown) {
	const errors: string[] = [];
	const permissions = asRecord(permissionsValue);
	if (!permissions) {
		return ["permissions.json must be an object."];
	}

	const declared = permissions.declared_permissions;
	if (!isStringArray(declared)) {
		errors.push("permissions.json declared_permissions must be an array of strings.");
	} else {
		errors.push(...validateUniqueKnownStrings(declared, permissionSet, "permission"));
		const recalculated = recalculateProgramDeclarations(programValue, manifestValue);
		errors.push(
			...compareStringSets(
				declared,
				recalculated.permissions.map((permission) => permission.name),
				"permissions.json declared_permissions",
			),
		);

		if (permissions.risk_level !== recalculated.riskLevel) {
			errors.push(`permissions.json risk_level must be ${recalculated.riskLevel} for the declared program.`);
		}
	}

	if (!isRiskLevel(permissions.risk_level)) {
		errors.push("permissions.json risk_level must be low, medium, high, or dangerous.");
	}

	return errors;
}

export function validateEditorContract(value: unknown) {
	if (value === undefined) {
		return [];
	}

	const errors: string[] = [];
	const editor = asRecord(value);
	if (!editor) {
		return ["editor.json must be an object when present."];
	}
	if (typeof editor.format_version !== "number" || editor.format_version < 1) {
		errors.push("editor.json format_version must be a positive number.");
	}
	if (typeof editor.created_with !== "string" || !editor.created_with.trim()) {
		errors.push("editor.json created_with must be a non-empty string.");
	}
	if (!Array.isArray(editor.nodes)) {
		errors.push("editor.json nodes must be an array.");
		return errors;
	}
	if (editor.canvas !== undefined) {
		const canvas = asRecord(editor.canvas);
		if (
			!canvas ||
			(canvas.edge_style !== undefined &&
				(typeof canvas.edge_style !== "string" || !isEditorEdgeStyle(canvas.edge_style)))
		) {
			errors.push("editor.json canvas.edge_style must be smoothstep, bezier, straight, or step when present.");
		}
	}

	for (const node of editor.nodes) {
		const record = asRecord(node);
		const position = asRecord(record?.position);
		if (
			!record ||
			typeof record.id !== "string" ||
			!position ||
			typeof position.x !== "number" ||
			typeof position.y !== "number" ||
			!Number.isFinite(position.x) ||
			!Number.isFinite(position.y)
		) {
			errors.push("editor.json nodes must contain id and finite position x/y values.");
			break;
		}
	}

	if (editor.comments !== undefined) {
		if (!Array.isArray(editor.comments)) {
			errors.push("editor.json comments must be an array when present.");
			return errors;
		}

		for (const comment of editor.comments) {
			const record = asRecord(comment);
			const position = asRecord(record?.position);
			const size = asRecord(record?.size);
			if (
				!record ||
				typeof record.id !== "string" ||
				typeof record.text !== "string" ||
				!isEditorCommentColor(record.color) ||
				(record.font_size !== undefined &&
					(typeof record.font_size !== "number" ||
						!Number.isFinite(record.font_size) ||
						record.font_size < 12 ||
						record.font_size > 72)) ||
				!position ||
				typeof position.x !== "number" ||
				typeof position.y !== "number" ||
				!Number.isFinite(position.x) ||
				!Number.isFinite(position.y) ||
				!size ||
				typeof size.width !== "number" ||
				typeof size.height !== "number" ||
				!Number.isFinite(size.width) ||
				!Number.isFinite(size.height) ||
				size.width <= 0 ||
				size.height <= 0
			) {
				errors.push(
					"editor.json comments must contain id, text, color, finite position x/y, positive size width/height values, and optional font_size from 12 to 72.",
				);
				break;
			}
		}
	}

	return errors;
}

function isEditorCommentColor(value: unknown) {
	return value === "amber" || value === "blue" || value === "green" || value === "rose" || value === "violet";
}

export function recalculateProgramDeclarations(programValue: unknown, manifestValue?: unknown) {
	const program = asRecord(programValue);
	const entry = asRecord(program?.entry);
	const block = asRecord(entry?.program);
	const triggers = Array.isArray(entry?.triggers) ? entry.triggers : [];
	const steps = Array.isArray(block?.steps) ? block.steps : [];
	const programNodes = [...triggers, ...steps]
		.map((node) => asRecord(node))
		.filter((node): node is Record<string, unknown> => {
			const actionType = node?.action_type;
			return typeof actionType === "string" && !!getNodeDefinition(actionType as ActionType);
		});
	const capabilities = new Set<string>();
	const permissions = new Map<string, PermissionSummary>();

	for (const node of programNodes) {
		const actionType = node.action_type as ActionType;
		const config = isJsonObject(node.config) ? node.config : {};
		for (const capability of getNodeCapabilities(actionType, config)) {
			capabilities.add(capability);
		}

		for (const permission of getNodePermissions(actionType, config)) {
			const existing = permissions.get(permission.name);
			if (!existing || riskWeight[permission.risk] > riskWeight[existing.risk]) {
				permissions.set(permission.name, permission);
			}
		}
	}

	const manifest = asRecord(manifestValue);
	if (Array.isArray(manifest?.secrets) && manifest.secrets.length > 0) {
		capabilities.add("runtime.secrets");
		permissions.set("read_secret", { name: "read_secret", risk: "high" });
	}
	if (Array.isArray(manifest?.variables) && manifest.variables.length > 0) {
		capabilities.add("runtime.variables");
		if (manifest.variables.some((value) => asRecord(value)?.scope === "runtime")) {
			permissions.set("set_local_variable", { name: "set_local_variable", risk: "low" });
		}
		if (manifest.variables.some((value) => asRecord(value)?.scope === "persistent")) {
			capabilities.add("runtime.persistent_storage");
			permissions.set("set_persistent_variable", { name: "set_persistent_variable", risk: "medium" });
		}
	}

	const permissionList = [...permissions.values()].sort(
		(a, b) => riskWeight[a.risk] - riskWeight[b.risk] || a.name.localeCompare(b.name),
	);

	return {
		capabilities: [...capabilities].sort(),
		permissions: permissionList,
		riskLevel: permissionList.reduce<RiskLevel>(
			(highest, permission) => (riskWeight[permission.risk] > riskWeight[highest] ? permission.risk : highest),
			"low",
		),
	};
}

function getProgramCompatibilityNodes(programValue: unknown) {
	const program = asRecord(programValue);
	const entry = asRecord(program?.entry);
	const block = asRecord(entry?.program);
	const triggers = Array.isArray(entry?.triggers) ? entry.triggers : [];
	const steps = Array.isArray(block?.steps) ? block.steps : [];

	return [...triggers, ...steps].flatMap((node) => {
		const record = asRecord(node);
		const actionType = typeof record?.action_type === "string" ? (record.action_type as ActionType) : undefined;
		if (!record || !actionType || !getNodeDefinition(actionType)) {
			return [];
		}

		return [
			{
				actionType,
				config: isJsonObject(record.config) ? record.config : undefined,
				id: typeof record.id === "string" && record.id.trim() ? record.id : actionType,
			},
		];
	});
}

function validateProgramNode(value: unknown, label: "step" | "trigger") {
	const errors: string[] = [];
	const node = asRecord(value);
	if (!node) {
		return [`program.json ${label} must be an object.`];
	}

	const actionType = typeof node.action_type === "string" ? (node.action_type as ActionType) : null;
	const definition = actionType ? getNodeDefinition(actionType) : undefined;
	if (!actionType || !definition) {
		return [`program.json ${label} has unsupported action_type ${String(node.action_type)}.`];
	}

	if (!isJsonObject(node.config)) {
		errors.push(`program.json node ${String(node.id ?? actionType)} config must be an object.`);
	} else {
		errors.push(
			...validateNodeConfig(actionType, node.config).map(
				(error) => `program.json node ${String(node.id ?? actionType)} config ${error}`,
			),
		);
	}
	if (!Array.isArray(node.runtime_outputs)) {
		errors.push(`program.json node ${String(node.id ?? actionType)} runtime_outputs must be an array.`);
	}

	if (label === "trigger") {
		if (!actionType.startsWith("trigger.")) {
			errors.push(`program.json trigger ${String(node.id ?? actionType)} must use a trigger action_type.`);
		} else if (node.type !== getRunnerTriggerType(actionType as Extract<ActionType, `trigger.${string}`>)) {
			errors.push(`program.json trigger ${String(node.id ?? actionType)} type does not match its action_type.`);
		}
		return errors;
	}

	if (actionType.startsWith("control.")) {
		if (node.type !== getControlStepType(actionType)) {
			errors.push(`program.json control step ${String(node.id ?? actionType)} type does not match its action_type.`);
		}
		return errors;
	}

	if (actionType === "runtime.set_variable") {
		if (node.type !== "set_variable") {
			errors.push(`program.json variable operation step ${String(node.id ?? actionType)} must use type set_variable.`);
		}
		return errors;
	}

	if (!actionType.startsWith("action.")) {
		errors.push(`program.json step ${String(node.id ?? actionType)} has invalid action_type.`);
	} else {
		if (node.type !== "action") {
			errors.push(`program.json action step ${String(node.id ?? actionType)} must use type action.`);
		}
		if (node.action !== getRunnerActionType(actionType as Extract<ActionType, `action.${string}`>)) {
			errors.push(`program.json action step ${String(node.id ?? actionType)} action does not match its action_type.`);
		}
	}

	return errors;
}

function validateUniqueKnownStrings(values: string[], allowed: ReadonlySet<string>, label: string) {
	const errors: string[] = [];
	const seen = new Set<string>();
	for (const value of values) {
		if (seen.has(value)) {
			errors.push(`Duplicate ${label} "${value}".`);
		}
		if (!allowed.has(value)) {
			errors.push(`Unknown ${label} "${value}".`);
		}
		seen.add(value);
	}
	return errors;
}

function compareStringSets(declared: string[], recalculated: string[], label: string) {
	const declaredSet = new Set(declared);
	const recalculatedSet = new Set(recalculated);
	const missing = recalculated.filter((value) => !declaredSet.has(value));
	const extra = declared.filter((value) => !recalculatedSet.has(value));
	const errors: string[] = [];

	if (missing.length > 0) {
		errors.push(`${label} is missing: ${missing.join(", ")}.`);
	}
	if (extra.length > 0) {
		errors.push(`${label} declares unused values: ${extra.join(", ")}.`);
	}

	return errors;
}

function asRecord(value: unknown): Record<string, unknown> | null {
	return value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : null;
}

function isStringArray(value: unknown): value is string[] {
	return Array.isArray(value) && value.every((entry) => typeof entry === "string" && entry.trim().length > 0);
}

function isRiskLevel(value: unknown): value is RiskLevel {
	return value === "low" || value === "medium" || value === "high" || value === "dangerous";
}

function isJsonObject(value: unknown): value is Record<string, JsonValue> {
	return !!asRecord(value) && Object.values(value as Record<string, unknown>).every(isJsonValue);
}

function isJsonValue(value: unknown): value is JsonValue {
	if (value === null || typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
		return true;
	}
	if (Array.isArray(value)) {
		return value.every(isJsonValue);
	}
	const record = asRecord(value);
	return !!record && Object.values(record).every(isJsonValue);
}

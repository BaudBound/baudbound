import type { Edge, Node, XYPosition } from "@xyflow/react";
import {
	Bot,
	Database,
	FileCog,
	FolderTree,
	Keyboard,
	MessageSquare,
	Network,
	Shapes,
	Terminal,
	Zap,
} from "lucide-react";
import { createReadFilePermission, createWriteFilePermission } from "@/data/project/file-permissions";
import type {
	ActionType,
	EditorAsset,
	ExecutableActionType,
	JsonValue,
	PaletteGroup,
	PaletteItem,
	ScriptNodeData,
	TargetRuntime,
	TriggerActionType,
} from "@/lib/types";
import { createGraphElementId } from "@/utils/graph-element-id";
import { isDesktopTargetRuntime } from "../project/runtimes";
import { beepNode } from "./definitions/actions/beep";
import { calculateNode } from "./definitions/actions/calculate";
import { clipboardNode } from "./definitions/actions/clipboard";
import { delayNode } from "./definitions/actions/delay";
import { copyFileNode } from "./definitions/actions/file-copy";
import { deleteFileNode } from "./definitions/actions/file-delete";
import { downloadFileNode } from "./definitions/actions/file-download";
import { moveFileNode } from "./definitions/actions/file-move";
import { readFileNode } from "./definitions/actions/file-read";
import { writeFileNode } from "./definitions/actions/file-write";
import { formatTextNode } from "./definitions/actions/format-text";
import { getActiveWindowNode } from "./definitions/actions/get-active-window";
import { getClipboardNode } from "./definitions/actions/get-clipboard";
import { getPixelColorNode } from "./definitions/actions/get-pixel-color";
import { httpRequestNode } from "./definitions/actions/http-request";
import { keyboardNode } from "./definitions/actions/keyboard";
import { killProcessNode } from "./definitions/actions/kill-process";
import { logNode } from "./definitions/actions/log";
import { messageBoxNode } from "./definitions/actions/message-box";
import { mouseClickNode } from "./definitions/actions/mouse-click";
import { moveMouseNode } from "./definitions/actions/move-mouse";
import { notificationNode } from "./definitions/actions/notification";
import { openApplicationNode } from "./definitions/actions/open-application";
import { playSoundNode } from "./definitions/actions/play-sound";
import { processStatusNode } from "./definitions/actions/process-status";
import { runProcessNode } from "./definitions/actions/run-process";
import { serialWriteNode } from "./definitions/actions/serial-write";
import { shellCommandNode } from "./definitions/actions/shell-command";
import { subScriptNode } from "./definitions/actions/sub-script";
import { typeTextNode } from "./definitions/actions/type-text";
import { variableOperationNode } from "./definitions/actions/variable-operation";
import { webhookResponseNode } from "./definitions/actions/webhook-response";
import { websocketWriteNode } from "./definitions/actions/websocket-write";
import { windowFocusNode } from "./definitions/actions/window-focus";
import { colorMatchNode } from "./definitions/control/color-match";
import { forEachNode } from "./definitions/control/for-each";
import { ifElseNode } from "./definitions/control/if-else";
import { loopNode } from "./definitions/control/loop";
import { switchNode } from "./definitions/control/switch";
import { whileNode } from "./definitions/control/while";
import { createSwitchOutputPorts, getSwitchCaseRowsFromValue } from "./definitions/rows";
import { fileWatchTriggerNode } from "./definitions/triggers/file-watch";
import { hotkeyTriggerNode } from "./definitions/triggers/hotkey";
import { manualTriggerNode } from "./definitions/triggers/manual";
import { processStartedTriggerNode } from "./definitions/triggers/process-started";
import { scheduleTriggerNode } from "./definitions/triggers/schedule";
import { serialInputTriggerNode } from "./definitions/triggers/serial-input";
import { startupTriggerNode } from "./definitions/triggers/startup";
import { webhookTriggerNode } from "./definitions/triggers/webhook";
import { websocketTriggerNode } from "./definitions/triggers/websocket";
import {
	defaultInputPort,
	defaultOutputPort,
	failureErrorOutput,
	fallibleActionOutputs,
	type NodeConfigField,
	type NodeDefinition,
	type NodeDefinitionGroupId,
	triggerOutputPort,
} from "./node-definition";
import { numericContractApplies, validateNumericConfigValue } from "./numeric-validation";

const nodeDefinitions: NodeDefinition[] = [
	manualTriggerNode,
	scheduleTriggerNode,
	fileWatchTriggerNode,
	webhookTriggerNode,
	websocketTriggerNode,
	hotkeyTriggerNode,
	serialInputTriggerNode,
	startupTriggerNode,
	processStartedTriggerNode,
	colorMatchNode,
	ifElseNode,
	switchNode,
	loopNode,
	whileNode,
	forEachNode,
	variableOperationNode,
	calculateNode,
	formatTextNode,
	logNode,
	delayNode,
	httpRequestNode,
	webhookResponseNode,
	websocketWriteNode,
	notificationNode,
	messageBoxNode,
	getPixelColorNode,
	readFileNode,
	writeFileNode,
	downloadFileNode,
	deleteFileNode,
	copyFileNode,
	moveFileNode,
	runProcessNode,
	processStatusNode,
	killProcessNode,
	subScriptNode,
	openApplicationNode,
	getActiveWindowNode,
	windowFocusNode,
	playSoundNode,
	serialWriteNode,
	keyboardNode,
	typeTextNode,
	mouseClickNode,
	moveMouseNode,
	beepNode,
	clipboardNode,
	getClipboardNode,
	shellCommandNode,
];

const groupMetadata: Record<NodeDefinitionGroupId, Pick<PaletteGroup, "id" | "label" | "icon">> = {
	triggers: { id: "triggers", label: "Triggers", icon: Zap },
	control: { id: "control", label: "Control Flow", icon: FolderTree },
	actions: { id: "actions", label: "Actions", icon: Shapes },
};

const riskSortOrder = {
	low: 0,
	medium: 1,
	high: 2,
	dangerous: 3,
};

const actionPaletteCategories = [
	{
		id: "actions-data",
		label: "Data & Variables",
		icon: Database,
		actionTypes: ["runtime.set_variable", "action.calculate", "action.text.format"] satisfies ActionType[],
	},
	{
		id: "actions-output",
		label: "Output & Timing",
		icon: MessageSquare,
		actionTypes: [
			"action.log",
			"action.delay",
			"action.beep",
			"action.notification",
			"action.message_box",
			"action.sound.play",
		] satisfies ActionType[],
	},
	{
		id: "actions-network",
		label: "Network & Serial",
		icon: Network,
		actionTypes: [
			"action.http",
			"action.webhook_response",
			"action.websocket.write",
			"action.serial.write",
		] satisfies ActionType[],
	},
	{
		id: "actions-files",
		label: "Files",
		icon: FileCog,
		actionTypes: [
			"action.file.read",
			"action.file.download",
			"action.file.copy",
			"action.file.move",
			"action.file.write",
			"action.file.delete",
		] satisfies ActionType[],
	},
	{
		id: "actions-window-process",
		label: "Windows & Processes",
		icon: Bot,
		actionTypes: [
			"action.window.active",
			"action.process.status",
			"action.application.open",
			"action.window.focus",
			"action.process.run",
			"action.process.kill",
		] satisfies ActionType[],
	},
	{
		id: "actions-input",
		label: "Input Control",
		icon: Keyboard,
		actionTypes: [
			"action.clipboard.set",
			"action.clipboard.get",
			"action.keyboard",
			"action.keyboard.type_text",
			"action.mouse",
			"action.mouse.move",
			"action.pixel.get",
		] satisfies ActionType[],
	},
	{
		id: "actions-scripts-system",
		label: "Scripts & System",
		icon: Terminal,
		actionTypes: ["action.script.run", "action.shell"] satisfies ActionType[],
	},
];

type CreateNodeOptions = {
	idPrefix?: string;
	baseX?: number;
	baseY?: number;
	columns?: number;
	columnGap?: number;
	position?: XYPosition;
	rowGap?: number;
};

export const nodeDefinitionByActionType = new Map<ActionType, NodeDefinition>(
	nodeDefinitions.map((definition) => [definition.actionType, definition]),
);

export const desktopOnlyActionTypes = new Set<ActionType>(
	nodeDefinitions.filter((definition) => definition.desktopOnly).map((definition) => definition.actionType),
);

export type TargetRuntimeCompatibilityNode = {
	actionType: ActionType;
	config?: Record<string, JsonValue>;
	id: string;
	label?: string;
};

export const fallibleActionTypes = new Set<ActionType>(
	nodeDefinitions.filter((definition) => definition.fallible).map((definition) => definition.actionType),
);

export function getNodeDefinition(actionType: ActionType) {
	return nodeDefinitionByActionType.get(actionType);
}

export function getTargetRuntimeCompatibilityErrors(
	nodes: TargetRuntimeCompatibilityNode[],
	targetRuntime: TargetRuntime,
) {
	return nodes.flatMap((node) => {
		const definition = getNodeDefinition(node.actionType);
		if (!definition) {
			return [];
		}

		const label = definition.label ?? node.label ?? node.actionType;
		const errors: string[] = [];

		if (definition.supportedTargetRuntimes && !definition.supportedTargetRuntimes.includes(targetRuntime)) {
			errors.push(
				`${node.id} (${label}) requires ${formatTargetRuntimeList(definition.supportedTargetRuntimes)}, but the script targets ${targetRuntime}.`,
			);
		}

		if (definition.desktopOnly && !isDesktopTargetRuntime(targetRuntime)) {
			errors.push(`${node.id} (${label}) requires a desktop target runtime, but the script targets ${targetRuntime}.`);
		}

		errors.push(
			...(definition
				.validateTargetRuntime?.({ config: node.config ?? {}, targetRuntime })
				.map((error) => `${node.id} (${label}) ${error}`) ?? []),
		);

		return errors;
	});
}

export function getPaletteGroups(): PaletteGroup[] {
	return (Object.keys(groupMetadata) as NodeDefinitionGroupId[]).map((groupId) => ({
		...groupMetadata[groupId],
		items: groupId === "actions" ? [] : createPaletteItemsForGroup(groupId),
		children: groupId === "actions" ? createActionPaletteChildren() : undefined,
	}));
}

function formatTargetRuntimeList(targetRuntimes: readonly TargetRuntime[]) {
	return targetRuntimes.length === 1
		? `${targetRuntimes[0]} target runtime`
		: `one of these target runtimes: ${targetRuntimes.join(", ")}`;
}

export function getFlatPaletteItems() {
	return getPaletteGroups().flatMap(flattenPaletteGroupItems);
}

export function createDefaultNodeConfig(actionType: ActionType): Record<string, JsonValue> {
	return getNodeDefinition(actionType)?.defaultConfig?.() ?? {};
}

function createPaletteItemsForGroup(groupId: NodeDefinitionGroupId) {
	return sortPaletteItems(nodeDefinitions.filter((definition) => definition.group === groupId).map(createPaletteItem));
}

function createActionPaletteChildren(): PaletteGroup[] {
	const actionDefinitions = new Map(
		nodeDefinitions
			.filter((definition) => definition.group === "actions")
			.map((definition) => [definition.actionType, definition]),
	);
	const categorizedActionTypes = new Set<ActionType>();

	const children = actionPaletteCategories
		.map((category) => {
			const items = sortPaletteItems(
				category.actionTypes.flatMap((actionType) => {
					const definition = actionDefinitions.get(actionType);
					if (!definition) {
						return [];
					}

					categorizedActionTypes.add(actionType);
					return [createPaletteItem(definition)];
				}),
			);

			return {
				id: category.id,
				label: category.label,
				icon: category.icon,
				items,
			};
		})
		.filter((category) => category.items.length > 0);

	const uncategorizedItems = sortPaletteItems(
		[...actionDefinitions.values()]
			.filter((definition) => !categorizedActionTypes.has(definition.actionType))
			.map(createPaletteItem),
	);

	if (uncategorizedItems.length > 0) {
		children.push({
			id: "actions-other",
			label: "Other",
			icon: Shapes,
			items: uncategorizedItems,
		});
	}

	return children;
}

function createPaletteItem(definition: NodeDefinition): PaletteItem {
	return {
		actionType: definition.actionType,
		description: definition.description,
		icon: definition.icon,
		kind: definition.kind,
		label: definition.label,
		risk: definition.risk,
	};
}

function sortPaletteItems(items: PaletteItem[]) {
	return [...items].sort((a, b) => riskSortOrder[a.risk] - riskSortOrder[b.risk] || a.label.localeCompare(b.label));
}

function flattenPaletteGroupItems(group: PaletteGroup): PaletteItem[] {
	return [...group.items, ...(group.children?.flatMap(flattenPaletteGroupItems) ?? [])];
}

export function createNodeFromPaletteItem(
	item: PaletteItem,
	index: number,
	{ idPrefix, baseX = 180, baseY = 120, columns = 4, columnGap = 260, position, rowGap = 130 }: CreateNodeOptions = {},
): Node<ScriptNodeData> {
	const column = index % columns;
	const row = Math.floor(index / columns);
	const config = createDefaultNodeConfig(item.actionType);
	const ports = getNodePorts(item.actionType, config);

	return {
		id: idPrefix ? `n-${idPrefix}-${index}` : createGraphElementId("n"),
		type: "scriptNode",
		position: position ?? {
			x: baseX + column * columnGap,
			y: baseY + row * rowGap,
		},
		data: {
			label: item.kind === "trigger" && !item.label.endsWith("Trigger") ? `${item.label} Trigger` : item.label,
			kind: item.kind,
			actionType: item.actionType,
			risk: item.risk,
			config,
			inputs: ports.inputs,
			outputs: ports.outputs,
			runtimeOutputs: getRuntimeDataOutputs(item.actionType),
		},
	};
}

export function getNodeConfigFields(actionType: ActionType) {
	return getNodeDefinition(actionType)?.configFields ?? [];
}

export function getNodePorts(actionType: ActionType, config?: Record<string, JsonValue>) {
	const definition = getNodeDefinition(actionType);
	if (definition?.portPolicy?.kind === "fixed") {
		return {
			inputs: definition.portPolicy.inputs.map((id) => ({ id, label: id })),
			outputs: definition.portPolicy.outputs.map((id) => ({ id, label: id })),
		};
	}
	if (definition?.portPolicy?.kind === "switch-cases") {
		return {
			inputs: [defaultInputPort],
			outputs: createSwitchOutputPorts(getSwitchCaseRowsFromValue(config?.[definition.portPolicy.configKey])),
		};
	}

	if (actionType.startsWith("trigger.")) {
		return { inputs: [], outputs: [triggerOutputPort] };
	}

	if (definition?.fallible) {
		return { inputs: [defaultInputPort], outputs: [...fallibleActionOutputs] };
	}

	return { inputs: [defaultInputPort], outputs: [defaultOutputPort] };
}

export function getRuntimeDataOutputs(actionType: ActionType) {
	const definition = getNodeDefinition(actionType);
	if (!definition) {
		return [];
	}

	if (definition.runtimeOutputs) {
		return definition.runtimeOutputs;
	}

	return definition.fallible ? [failureErrorOutput] : [];
}

export function getNodeCapabilities(actionType: ActionType, config: Record<string, JsonValue> = {}) {
	const definition = getNodeDefinition(actionType);
	return definition?.deriveCapabilities?.(config) ?? definition?.capabilities ?? [];
}

export function getNodePermission(actionType: ActionType) {
	return getNodeDefinition(actionType)?.permission;
}

export function getNodePermissions(actionType: ActionType, config: Record<string, JsonValue> = {}) {
	const definition = getNodeDefinition(actionType);
	if (!definition) {
		return [];
	}

	if (definition.derivePermissions) {
		return definition.derivePermissions(config);
	}

	const pathRules = definition.permissionPathRules ?? [];
	const replacesBasePermission = pathRules.some(
		(rule) =>
			(rule.access === "read" && definition.permission?.name === "file_read") ||
			(rule.access === "write" && definition.permission?.name === "file_write_limited"),
	);
	const permissions = definition.permission && !replacesBasePermission ? [definition.permission] : [];
	for (const rule of pathRules) {
		permissions.push(
			rule.access === "read"
				? createReadFilePermission(config[rule.configKey])
				: createWriteFilePermission(config[rule.configKey]),
		);
	}

	return [...new Map(permissions.map((permission) => [permission.name, permission])).values()];
}

export function getRunnerTriggerType(actionType: TriggerActionType) {
	return getRequiredRunnerType(actionType);
}

export function getRunnerActionType(actionType: ExecutableActionType) {
	return getRequiredRunnerType(actionType);
}

export function getControlStepType(actionType: ActionType) {
	const controlType = getNodeDefinition(actionType)?.controlType;
	if (!controlType) {
		throw new Error(`Unsupported control flow action type in export: ${actionType}`);
	}

	return controlType;
}

export function sanitizeNodeConfig(actionType: ActionType, config: Record<string, JsonValue>) {
	const definition = getNodeDefinition(actionType);
	if (!definition) {
		return config;
	}

	if (definition.sanitizeConfig) {
		return definition.sanitizeConfig(config);
	}

	const allowedKeys = getAllowedNodeConfigKeys(definition);
	return Object.fromEntries(Object.entries(config).filter(([key]) => allowedKeys.has(key)));
}

export function validateNodeConfigKeys(actionType: ActionType, config: Record<string, JsonValue>) {
	const definition = getNodeDefinition(actionType);
	if (!definition) {
		return [`Unsupported action type: ${actionType}.`];
	}

	const allowedKeys = getAllowedNodeConfigKeys(definition);
	const unknownKeys = Object.keys(config).filter((key) => !allowedKeys.has(key));

	return unknownKeys.length > 0
		? [`Unknown config field${unknownKeys.length === 1 ? "" : "s"}: ${unknownKeys.join(", ")}.`]
		: [];
}

export function validateNodeConfig(actionType: ActionType, config: Record<string, JsonValue>) {
	const definition = getNodeDefinition(actionType);
	const sanitizedConfig = sanitizeNodeConfig(actionType, config);
	const keyErrors = validateNodeConfigKeys(actionType, sanitizedConfig);

	return [
		...keyErrors,
		...(definition ? validateDeclaredConfigFields(definition.configFields ?? [], sanitizedConfig) : []),
		...(definition?.validateConfig?.(sanitizedConfig) ?? []),
	];
}

export function validateNodeGraph(
	node: Node<ScriptNodeData>,
	context: { assets: EditorAsset[]; edges: Edge[]; nodes: Node<ScriptNodeData>[] },
) {
	return getNodeDefinition(node.data.actionType)?.validateGraph?.({ context, node }) ?? [];
}

function getAllowedNodeConfigKeys(definition: NodeDefinition) {
	const defaultConfig = definition.defaultConfig?.() ?? {};

	return new Set([
		"customName",
		...Object.keys(defaultConfig),
		...(definition.configFields ?? []).map((field) => field.key),
	]);
}

function validateDeclaredConfigFields(fields: NodeConfigField[], config: Record<string, JsonValue>) {
	return fields.flatMap((field) => {
		if (field.required !== false && !(field.key in config)) {
			return [`Missing required config field: ${field.key}.`];
		}

		if (!(field.key in config)) {
			return [];
		}

		const value = config[field.key];
		if (field.required === false && typeof value === "string" && !value.trim()) {
			return [];
		}

		if ((field.type === "text" || field.type === "textarea") && typeof value !== "string") {
			return [`Invalid value for ${field.key}: expected string.`];
		}

		if (field.type === "select" && field.options && typeof value === "string") {
			const allowedValues = new Set(field.options.map((option) => option.value));
			if (!allowedValues.has(value)) {
				return [`Invalid value for ${field.key}: ${value}. Expected one of ${[...allowedValues].join(", ")}.`];
			}
		}

		if (field.type === "select" && typeof value !== "string") {
			return [`Invalid value for ${field.key}: expected string.`];
		}

		if (numericContractApplies(field, config)) {
			if (!field.numeric) {
				throw new Error(`Numeric contract metadata is missing for ${field.key}.`);
			}
			if (field.usesVariables === true && typeof value === "string" && /\{\{[^}]+\}\}/.test(value)) {
				return [];
			}
			const error = validateNumericConfigValue(value, field.numeric);
			if (error) {
				return [`Invalid value for ${field.key}: ${error}.`];
			}
		}

		if (field.type === "switch" && typeof value !== "boolean") {
			return [`Invalid value for ${field.key}: expected boolean.`];
		}

		return [];
	});
}

function getRequiredRunnerType(actionType: ActionType) {
	const runnerType = getNodeDefinition(actionType)?.runnerType;
	if (!runnerType) {
		throw new Error(`Unsupported action type in export: ${actionType}`);
	}

	return runnerType;
}

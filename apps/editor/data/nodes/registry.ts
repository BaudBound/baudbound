import type { Node, XYPosition } from "@xyflow/react";
import { FolderTree, Shapes, Zap } from "lucide-react";
import type {
	ActionType,
	ExecutableActionType,
	JsonValue,
	PaletteGroup,
	PaletteItem,
	ScriptNodeData,
	TriggerActionType,
} from "@/lib/types";
import { beepNode } from "./definitions/actions/beep";
import { calculateNode } from "./definitions/actions/calculate";
import { clipboardNode } from "./definitions/actions/clipboard";
import { delayNode } from "./definitions/actions/delay";
import { copyFileNode } from "./definitions/actions/file-copy";
import { deleteFileNode } from "./definitions/actions/file-delete";
import { downloadFileNode } from "./definitions/actions/file-download";
import { moveFileNode } from "./definitions/actions/file-move";
import { writeFileNode } from "./definitions/actions/file-write";
import { formatTextNode } from "./definitions/actions/format-text";
import { getActiveWindowNode } from "./definitions/actions/get-active-window";
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
import { setVariableNode } from "./definitions/actions/set-variable";
import { shellCommandNode } from "./definitions/actions/shell-command";
import { subScriptNode } from "./definitions/actions/sub-script";
import { typeTextNode } from "./definitions/actions/type-text";
import { windowFocusNode } from "./definitions/actions/window-focus";
import { forEachNode } from "./definitions/control/for-each";
import { ifElseNode } from "./definitions/control/if-else";
import { loopNode } from "./definitions/control/loop";
import { switchNode } from "./definitions/control/switch";
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
	type NodeDefinition,
	type NodeDefinitionGroupId,
	triggerOutputPort,
} from "./node-definition";

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
	ifElseNode,
	switchNode,
	loopNode,
	forEachNode,
	setVariableNode,
	calculateNode,
	formatTextNode,
	logNode,
	delayNode,
	httpRequestNode,
	notificationNode,
	messageBoxNode,
	getPixelColorNode,
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
	shellCommandNode,
];

const groupMetadata: Record<NodeDefinitionGroupId, Pick<PaletteGroup, "id" | "label" | "icon">> = {
	triggers: { id: "triggers", label: "Triggers", icon: Zap },
	control: { id: "control", label: "Control Flow", icon: FolderTree },
	actions: { id: "actions", label: "Actions", icon: Shapes },
};

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

export const fallibleActionTypes = new Set<ActionType>(
	nodeDefinitions.filter((definition) => definition.fallible).map((definition) => definition.actionType),
);

export function getNodeDefinition(actionType: ActionType) {
	return nodeDefinitionByActionType.get(actionType);
}

export function getPaletteGroups(): PaletteGroup[] {
	return (Object.keys(groupMetadata) as NodeDefinitionGroupId[]).map((groupId) => ({
		...groupMetadata[groupId],
		items: nodeDefinitions
			.filter((definition) => definition.group === groupId)
			.map(
				(definition): PaletteItem => ({
					actionType: definition.actionType,
					description: definition.description,
					icon: definition.icon,
					kind: definition.kind,
					label: definition.label,
					risk: definition.risk,
				}),
			),
	}));
}

export function createDefaultNodeConfig(actionType: ActionType): Record<string, JsonValue> {
	return getNodeDefinition(actionType)?.defaultConfig?.() ?? {};
}

export function createNodeFromPaletteItem(
	item: PaletteItem,
	index: number,
	{
		idPrefix = Date.now().toString(36),
		baseX = 180,
		baseY = 120,
		columns = 4,
		columnGap = 260,
		position,
		rowGap = 130,
	}: CreateNodeOptions = {},
): Node<ScriptNodeData> {
	const column = index % columns;
	const row = Math.floor(index / columns);
	const config = createDefaultNodeConfig(item.actionType);
	const ports = getNodePorts(item.actionType, config);

	return {
		id: `n-${idPrefix}-${index}`,
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
	if (definition?.ports) {
		return definition.ports(config);
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

export function getNodeCapabilities(actionType: ActionType) {
	return getNodeDefinition(actionType)?.capabilities ?? [];
}

export function getNodePermission(actionType: ActionType) {
	return getNodeDefinition(actionType)?.permission;
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
	return getNodeDefinition(actionType)?.sanitizeConfig?.(config) ?? config;
}

function getRequiredRunnerType(actionType: ActionType) {
	const runnerType = getNodeDefinition(actionType)?.runnerType;
	if (!runnerType) {
		throw new Error(`Unsupported action type in export: ${actionType}`);
	}

	return runnerType;
}

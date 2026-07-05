import type { Edge, Node, XYPosition } from "@xyflow/react";
import type { EditorAsset, ProjectSettings, ScriptNodeData } from "@/lib/types";

const SCRIPT_NODE_WIDTH = 256;
const SCRIPT_NODE_ESTIMATED_HEIGHT = 144;

export const DUPLICATE_OFFSET = 44;

export function createGraphNodeCopy<NodeType extends Node>(sourceNode: NodeType, position: XYPosition): NodeType {
	const idPrefix = sourceNode.type === "commentNode" ? "c" : "n";

	return {
		...cloneGraphValue(sourceNode),
		id: `${idPrefix}-${createGraphElementId()}`,
		position,
		selected: false,
		dragging: false,
		data: cloneGraphValue(sourceNode.data),
	} as NodeType;
}

export function getCenteredScriptNodePosition(center: XYPosition): XYPosition {
	return {
		x: center.x - SCRIPT_NODE_WIDTH / 2,
		y: center.y - SCRIPT_NODE_ESTIMATED_HEIGHT / 2,
	};
}

export function cloneGraphValue<Value>(value: Value): Value {
	return structuredClone(value);
}

export function hasManualTrigger(nodes: Node<ScriptNodeData>[]) {
	return nodes.some((node) => node.data.actionType === "trigger.manual");
}

export function createEditorVerificationSignature(
	projectSettings: ProjectSettings,
	nodes: Node<ScriptNodeData>[],
	edges: Edge[],
	assets: EditorAsset[],
) {
	return JSON.stringify({
		projectSettings,
		assets: assets.map((asset) => ({
			id: asset.id,
			kind: asset.kind,
			mediaType: asset.mediaType,
			name: asset.name,
			packagePath: asset.packagePath,
			size: asset.size,
		})),
		nodes: nodes.map((node) => ({
			id: node.id,
			type: node.type,
			position: node.position,
			data: node.data,
		})),
		edges: edges.map((edge) => ({
			id: edge.id,
			source: edge.source,
			sourceHandle: edge.sourceHandle,
			target: edge.target,
			targetHandle: edge.targetHandle,
		})),
	});
}

function createGraphElementId() {
	return `${Date.now().toString(36)}-${crypto.randomUUID().slice(0, 8)}`;
}

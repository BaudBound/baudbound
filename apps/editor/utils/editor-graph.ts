import type { Edge, Node, XYPosition } from "@xyflow/react";
import type { DefaultVariable, EditorAsset, ProjectSettings, ScriptNodeData, SecretDeclaration } from "@/lib/types";
import { createGraphElementId } from "./graph-element-id";

const SCRIPT_NODE_WIDTH = 256;
const SCRIPT_NODE_ESTIMATED_HEIGHT = 144;

export const DUPLICATE_OFFSET = 44;

export type GraphFragment<NodeType extends Node = Node, EdgeType extends Edge = Edge> = {
	edges: EdgeType[];
	nodes: NodeType[];
};

const EXECUTION_ORDER_DATA_KEY = "executionOrder";

export function createGraphNodeCopy<NodeType extends Node>(sourceNode: NodeType, position: XYPosition): NodeType {
	const idPrefix = sourceNode.type === "commentNode" ? "c" : "n";

	return {
		...cloneGraphValue(sourceNode),
		id: createGraphElementId(idPrefix),
		position,
		selected: false,
		dragging: false,
		data: cloneGraphValue(sourceNode.data),
	} as NodeType;
}

export function createGraphFragment<NodeType extends Node, EdgeType extends Edge>(
	nodes: NodeType[],
	edges: EdgeType[],
): GraphFragment<NodeType, EdgeType> {
	const nodeIds = new Set(nodes.map((node) => node.id));

	return {
		nodes: cloneGraphValue(nodes),
		edges: normalizeEdgeExecutionOrders(
			cloneGraphValue(edges.filter((edge) => nodeIds.has(edge.source) && nodeIds.has(edge.target))),
		),
	};
}

export function createGraphFragmentCopy<NodeType extends Node, EdgeType extends Edge>(
	fragment: GraphFragment<NodeType, EdgeType>,
	center: XYPosition,
): GraphFragment<NodeType, EdgeType> {
	const bounds = getGraphFragmentBounds(fragment.nodes);
	const offset = {
		x: center.x - (bounds.x + bounds.width / 2),
		y: center.y - (bounds.y + bounds.height / 2),
	};
	const copiedNodeIds = new Map<string, string>();
	const nodes = fragment.nodes.map((node) => {
		const sourcePosition = getGraphNodePosition(node);
		const copiedNode = createGraphNodeCopy(node, {
			x: sourcePosition.x + offset.x,
			y: sourcePosition.y + offset.y,
		});
		copiedNode.selected = true;
		copiedNodeIds.set(node.id, copiedNode.id);
		return copiedNode;
	});
	const edges = fragment.edges.flatMap((edge) => {
		const source = copiedNodeIds.get(edge.source);
		const target = copiedNodeIds.get(edge.target);
		if (!source || !target) {
			return [];
		}

		const copiedEdge = cloneGraphValue(edge);
		return [
			{
				...copiedEdge,
				id: createGraphEdgeId({
					source,
					sourceHandle: copiedEdge.sourceHandle,
					target,
					targetHandle: copiedEdge.targetHandle,
				}),
				selected: true,
				source,
				target,
			} as EdgeType,
		];
	});

	return { edges, nodes };
}

export function createGraphEdgeId({
	source,
	sourceHandle,
	target,
	targetHandle,
}: Pick<Edge, "source" | "sourceHandle" | "target" | "targetHandle">) {
	return `${source}-${sourceHandle ?? "out"}-${target}-${targetHandle ?? "input"}`;
}

export function isSelfConnection({ source, target }: Pick<Edge, "source" | "target">) {
	return source === target;
}

export function getEdgeExecutionOrder(edge: Edge) {
	const value = edge.data?.[EXECUTION_ORDER_DATA_KEY];
	return typeof value === "number" && Number.isSafeInteger(value) && value >= 0 ? value : null;
}

export function withEdgeExecutionOrder<EdgeType extends Edge>(edge: EdgeType, executionOrder: number): EdgeType {
	return {
		...edge,
		data: {
			...edge.data,
			[EXECUTION_ORDER_DATA_KEY]: executionOrder,
		},
	} as EdgeType;
}

export function getOrderedExecutionGroup(edges: Edge[], source: string, sourceHandle: string | null | undefined) {
	return edges
		.filter((edge) => edge.source === source && edge.sourceHandle === sourceHandle)
		.sort(compareEdgeExecutionOrder);
}

export function getNextEdgeExecutionOrder(edges: Edge[], source: string, sourceHandle: string | null | undefined) {
	return getOrderedExecutionGroup(edges, source, sourceHandle).length;
}

export function normalizeEdgeExecutionOrders<EdgeType extends Edge>(edges: EdgeType[]): EdgeType[] {
	const groups = new Map<string, EdgeType[]>();
	for (const edge of edges) {
		const key = executionGroupKey(edge.source, edge.sourceHandle);
		const group = groups.get(key) ?? [];
		group.push(edge);
		groups.set(key, group);
	}

	const orders = new Map<string, number>();
	for (const group of groups.values()) {
		group.sort(compareEdgeExecutionOrder).forEach((edge, index) => {
			orders.set(edge.id, index);
		});
	}

	return edges.map((edge) => withEdgeExecutionOrder(edge, orders.get(edge.id) ?? 0));
}

export function reorderEdgeExecutionGroup<EdgeType extends Edge>(
	edges: EdgeType[],
	orderedEdgeIds: string[],
): EdgeType[] {
	const firstEdge = edges.find((edge) => edge.id === orderedEdgeIds[0]);
	if (!firstEdge) {
		return edges;
	}

	const group = getOrderedExecutionGroup(edges, firstEdge.source, firstEdge.sourceHandle);
	if (
		group.length !== orderedEdgeIds.length ||
		group.some((edge) => !orderedEdgeIds.includes(edge.id)) ||
		new Set(orderedEdgeIds).size !== orderedEdgeIds.length
	) {
		return edges;
	}

	const orders = new Map(orderedEdgeIds.map((id, index) => [id, index]));
	return edges.map((edge) => {
		const order = orders.get(edge.id);
		return order === undefined ? edge : withEdgeExecutionOrder(edge, order);
	});
}

export function getEdgeExecutionOrderErrors(edges: Edge[]) {
	const errors: string[] = [];
	const groups = new Map<string, Edge[]>();
	for (const edge of edges) {
		if (getEdgeExecutionOrder(edge) === null) {
			errors.push(`Connection ${edge.id} must define a non-negative integer execution order.`);
		}
		const key = executionGroupKey(edge.source, edge.sourceHandle);
		const group = groups.get(key) ?? [];
		group.push(edge);
		groups.set(key, group);
	}

	for (const group of groups.values()) {
		const orders = group.map(getEdgeExecutionOrder);
		if (orders.some((order) => order === null)) {
			continue;
		}
		const sortedOrders = (orders as number[]).sort((left, right) => left - right);
		if (sortedOrders.some((order, index) => order !== index)) {
			const first = group[0];
			errors.push(
				`Connections from ${first.source} output "${first.sourceHandle ?? ""}" must use unique consecutive execution orders starting at 0.`,
			);
		}
	}

	return errors;
}

function compareEdgeExecutionOrder(left: Edge, right: Edge) {
	const leftOrder = getEdgeExecutionOrder(left) ?? Number.MAX_SAFE_INTEGER;
	const rightOrder = getEdgeExecutionOrder(right) ?? Number.MAX_SAFE_INTEGER;
	return leftOrder - rightOrder || left.id.localeCompare(right.id);
}

function executionGroupKey(source: string, sourceHandle: string | null | undefined) {
	return `${source}\u0000${sourceHandle ?? ""}`;
}

function getGraphFragmentBounds(nodes: Node[]) {
	const firstNode = nodes[0];
	if (!firstNode) {
		return { height: 0, width: 0, x: 0, y: 0 };
	}

	let minX = Number.POSITIVE_INFINITY;
	let minY = Number.POSITIVE_INFINITY;
	let maxX = Number.NEGATIVE_INFINITY;
	let maxY = Number.NEGATIVE_INFINITY;
	for (const node of nodes) {
		const position = getGraphNodePosition(node);
		const size = getGraphNodeSize(node);
		minX = Math.min(minX, position.x);
		minY = Math.min(minY, position.y);
		maxX = Math.max(maxX, position.x + size.width);
		maxY = Math.max(maxY, position.y + size.height);
	}

	return {
		height: maxY - minY,
		width: maxX - minX,
		x: minX,
		y: minY,
	};
}

function getGraphNodePosition(node: Node): XYPosition {
	return {
		x: Number.isFinite(node.position?.x) ? node.position.x : 0,
		y: Number.isFinite(node.position?.y) ? node.position.y : 0,
	};
}

function getGraphNodeSize(node: Node) {
	return {
		width: getFiniteDimension(
			node.measured?.width,
			node.width,
			node.initialWidth,
			node.style?.width,
			SCRIPT_NODE_WIDTH,
		),
		height: getFiniteDimension(
			node.measured?.height,
			node.height,
			node.initialHeight,
			node.style?.height,
			SCRIPT_NODE_ESTIMATED_HEIGHT,
		),
	};
}

function getFiniteDimension(...values: unknown[]) {
	for (const value of values) {
		const dimension =
			typeof value === "number" ? value : typeof value === "string" ? Number.parseFloat(value) : Number.NaN;
		if (Number.isFinite(dimension) && dimension > 0) {
			return dimension;
		}
	}

	return 0;
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
	secretDeclarations: SecretDeclaration[] = [],
	defaultVariables: DefaultVariable[] = [],
) {
	return JSON.stringify({
		projectSettings,
		secretDeclarations,
		defaultVariables,
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

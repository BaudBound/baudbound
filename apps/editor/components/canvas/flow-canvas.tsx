"use client";

import {
	addEdge,
	Background,
	type Connection,
	Controls,
	type Edge,
	MiniMap,
	type Node,
	type OnEdgesChange,
	type OnNodesChange,
	ReactFlow,
	ReactFlowProvider,
	useReactFlow,
	type XYPosition,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { Grid3X3 } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { defaultEdgeOptions, edgeColors, reactFlowProOptions } from "@/data/editor/flow-canvas";
import type { ScriptNodeData } from "@/lib/types";
import { CanvasContextMenu, type CanvasContextMenuState } from "./canvas-context-menu";
import { ScriptNode } from "./script-node";

type FlowCanvasProps = {
	nodes: Node<ScriptNodeData>[];
	edges: Edge[];
	selectedEdgeId: string | null;
	onNodesChange: OnNodesChange<Node<ScriptNodeData>>;
	onEdgesChange: OnEdgesChange<Edge>;
	onEdgesCommit: (updater: (edges: Edge[]) => Edge[]) => void;
	onNodesDelete: (deletedNodes: Node<ScriptNodeData>[]) => void;
	onSelectNode: (nodeId: string | null) => void;
	onSelectEdge: (edgeId: string | null) => void;
	onEdgesDelete: (deletedEdges: Edge[]) => void;
	canPaste: boolean;
	onCopyNode: (nodeId: string) => void;
	onDeleteNode: (nodeId: string) => void;
	onDeleteEdge: (edgeId: string) => void;
	onDuplicateNode: (nodeId: string) => void;
	onPaste: (position: XYPosition) => void;
	onSpawnDevelopmentNodes?: () => void;
	showDevelopmentNodeSpawner?: boolean;
	onViewportCenterChange: (position: XYPosition) => void;
};

const nodeTypes = {
	scriptNode: ScriptNode,
};

type ContextMenuEvent = {
	clientX: number;
	clientY: number;
	preventDefault: () => void;
	stopPropagation?: () => void;
};

export function FlowCanvas({
	canPaste,
	edges,
	nodes,
	onCopyNode,
	onDeleteEdge,
	onDeleteNode,
	onDuplicateNode,
	onEdgesChange,
	onEdgesCommit,
	onEdgesDelete,
	onNodesChange,
	onNodesDelete,
	onPaste,
	onSpawnDevelopmentNodes,
	onSelectEdge,
	onSelectNode,
	showDevelopmentNodeSpawner,
	onViewportCenterChange,
	selectedEdgeId,
}: FlowCanvasProps) {
	return (
		<ReactFlowProvider>
			<FlowCanvasContent
				canPaste={canPaste}
				edges={edges}
				nodes={nodes}
				onCopyNode={onCopyNode}
				onDeleteEdge={onDeleteEdge}
				onDeleteNode={onDeleteNode}
				onDuplicateNode={onDuplicateNode}
				onEdgesChange={onEdgesChange}
				onEdgesCommit={onEdgesCommit}
				onEdgesDelete={onEdgesDelete}
				onNodesChange={onNodesChange}
				onNodesDelete={onNodesDelete}
				onPaste={onPaste}
				onSpawnDevelopmentNodes={onSpawnDevelopmentNodes}
				onSelectEdge={onSelectEdge}
				onSelectNode={onSelectNode}
				showDevelopmentNodeSpawner={showDevelopmentNodeSpawner}
				onViewportCenterChange={onViewportCenterChange}
				selectedEdgeId={selectedEdgeId}
			/>
		</ReactFlowProvider>
	);
}

function FlowCanvasContent({
	nodes,
	edges,
	selectedEdgeId,
	onNodesChange,
	onEdgesChange,
	onEdgesCommit,
	onNodesDelete,
	onSelectNode,
	onSelectEdge,
	onEdgesDelete,
	canPaste,
	onCopyNode,
	onDeleteNode,
	onDeleteEdge,
	onDuplicateNode,
	onPaste,
	onSpawnDevelopmentNodes,
	showDevelopmentNodeSpawner,
	onViewportCenterChange,
}: FlowCanvasProps) {
	const { screenToFlowPosition } = useReactFlow<Node<ScriptNodeData>, Edge>();
	const viewportRef = useRef<HTMLDivElement>(null);
	const initFrameRef = useRef<number | null>(null);
	const lastViewportCenterRef = useRef<XYPosition | null>(null);
	const [contextMenu, setContextMenu] = useState<CanvasContextMenuState | null>(null);

	const onConnect = useCallback(
		(connection: Connection) => {
			const edgeId = `${connection.source}-${connection.sourceHandle ?? "out"}-${connection.target}-${
				connection.targetHandle ?? "input"
			}`;
			onEdgesCommit((current) => addEdge({ ...connection, id: edgeId, type: "smoothstep" }, current));
			onSelectNode(null);
			onSelectEdge(edgeId);
		},
		[onEdgesCommit, onSelectEdge, onSelectNode],
	);

	const closeContextMenu = useCallback(() => setContextMenu(null), []);

	const updateViewportCenter = useCallback(() => {
		const viewport = viewportRef.current;
		if (!viewport) {
			return;
		}

		const bounds = viewport.getBoundingClientRect();
		const nextCenter = screenToFlowPosition({
			x: bounds.left + bounds.width / 2,
			y: bounds.top + bounds.height / 2,
		});
		const previousCenter = lastViewportCenterRef.current;
		if (
			previousCenter &&
			Math.abs(previousCenter.x - nextCenter.x) < 0.5 &&
			Math.abs(previousCenter.y - nextCenter.y) < 0.5
		) {
			return;
		}

		lastViewportCenterRef.current = nextCenter;
		onViewportCenterChange(nextCenter);
	}, [onViewportCenterChange, screenToFlowPosition]);

	useEffect(() => {
		window.addEventListener("contextmenu", closeContextMenu, { capture: true });

		return () => window.removeEventListener("contextmenu", closeContextMenu, { capture: true });
	}, [closeContextMenu]);

	useEffect(() => {
		const viewport = viewportRef.current;
		if (!viewport) {
			return;
		}

		const resizeObserver = new ResizeObserver(updateViewportCenter);
		resizeObserver.observe(viewport);
		const frame = window.requestAnimationFrame(updateViewportCenter);

		return () => {
			window.cancelAnimationFrame(frame);
			resizeObserver.disconnect();
		};
	}, [updateViewportCenter]);

	const openContextMenu = useCallback(
		(event: ContextMenuEvent, target: CanvasContextMenuState["target"]) => {
			event.preventDefault();
			event.stopPropagation?.();
			setContextMenu({
				x: event.clientX,
				y: event.clientY,
				flowPosition: screenToFlowPosition({ x: event.clientX, y: event.clientY }),
				target,
			});
		},
		[screenToFlowPosition],
	);

	useEffect(() => {
		if (!contextMenu) {
			return;
		}

		const handleKeyDown = (event: KeyboardEvent) => {
			if (event.key === "Escape") {
				closeContextMenu();
			}
		};

		window.addEventListener("keydown", handleKeyDown);

		return () => window.removeEventListener("keydown", handleKeyDown);
	}, [closeContextMenu, contextMenu]);

	useEffect(() => {
		return () => {
			if (initFrameRef.current !== null) {
				window.cancelAnimationFrame(initFrameRef.current);
				initFrameRef.current = null;
			}
		};
	}, []);

	const displayedEdges = useMemo(
		() =>
			edges.map((edge) => {
				const selected = edge.id === selectedEdgeId;

				return {
					...edge,
					className: selected ? "baud-edge-selected" : undefined,
					style: {
						...edge.style,
						stroke: selected ? edgeColors.selected : edgeColors.default,
						strokeWidth: selected ? 4 : 2,
					},
				};
			}),
		[edges, selectedEdgeId],
	);

	return (
		<div ref={viewportRef} className="relative min-h-0 flex-1 bg-baud-canvas">
			<ReactFlow
				nodes={nodes}
				edges={displayedEdges}
				nodeTypes={nodeTypes}
				onNodesChange={onNodesChange}
				onEdgesChange={onEdgesChange}
				onConnect={onConnect}
				onNodeClick={(_, node) => {
					closeContextMenu();
					onSelectEdge(null);
					onSelectNode(node.id);
				}}
				onNodeContextMenu={(event, node) => {
					onSelectEdge(null);
					onSelectNode(node.id);
					openContextMenu(event, { type: "node", id: node.id });
				}}
				onEdgeClick={(event, edge) => {
					event.stopPropagation();
					closeContextMenu();
					onSelectNode(null);
					onSelectEdge(edge.id);
				}}
				onEdgeContextMenu={(event, edge) => {
					onSelectNode(null);
					onSelectEdge(edge.id);
					openContextMenu(event, { type: "edge", id: edge.id });
				}}
				onPaneClick={() => {
					closeContextMenu();
					onSelectNode(null);
					onSelectEdge(null);
				}}
				onPaneContextMenu={(event) => {
					onSelectNode(null);
					onSelectEdge(null);
					openContextMenu(event, { type: "pane" });
				}}
				onInit={() => {
					if (initFrameRef.current !== null) {
						window.cancelAnimationFrame(initFrameRef.current);
					}
					initFrameRef.current = window.requestAnimationFrame(() => {
						initFrameRef.current = null;
						updateViewportCenter();
					});
				}}
				onMoveStart={closeContextMenu}
				onMoveEnd={updateViewportCenter}
				onNodesDelete={onNodesDelete}
				onEdgesDelete={onEdgesDelete}
				deleteKeyCode={["Backspace", "Delete"]}
				edgesFocusable
				elementsSelectable
				nodesFocusable
				minZoom={0.02}
				fitView
				fitViewOptions={{ padding: 0.25 }}
				defaultEdgeOptions={defaultEdgeOptions}
				proOptions={reactFlowProOptions}
			>
				<Background color="#25304a" gap={22} size={1.2} />
				<Controls
					className="baud-flow-controls"
					position="bottom-left"
					showInteractive={false}
					fitViewOptions={{ padding: 0.25 }}
				/>
				<MiniMap
					className="baud-flow-minimap"
					position="bottom-right"
					bgColor="#080a0f"
					nodeBorderRadius={4}
					nodeColor="#171b27"
					nodeStrokeColor="#53627d"
					maskColor="rgb(8 10 15 / 62%)"
					pannable
					zoomable
				/>
			</ReactFlow>
			{showDevelopmentNodeSpawner && onSpawnDevelopmentNodes && (
				<div className="pointer-events-none absolute top-4 right-4 z-10">
					<Button
						type="button"
						onClick={onSpawnDevelopmentNodes}
						className="pointer-events-auto"
						size="sm"
						variant="toolbar"
					>
						<Grid3X3 size={14} />
						Spawn dev nodes
					</Button>
				</div>
			)}
			{contextMenu && (
				<CanvasContextMenu
					canPaste={canPaste}
					menu={contextMenu}
					onClose={closeContextMenu}
					onCopyNode={onCopyNode}
					onDeleteNode={onDeleteNode}
					onDeleteEdge={onDeleteEdge}
					onDuplicateNode={onDuplicateNode}
					onPaste={onPaste}
				/>
			)}
		</div>
	);
}

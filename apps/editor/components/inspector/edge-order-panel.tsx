import type { Edge, Node } from "@xyflow/react";
import { ArrowDown, ArrowUp, GripVertical, Unlink2 } from "lucide-react";
import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { type ActiveReorderDragState, useReorderController } from "@/hooks/use-reorder-controller";
import type { ScriptNodeData } from "@/lib/types";
import { getOrderedExecutionGroup } from "@/utils/editor-graph";

type EdgeOrderRow = {
	id: string;
	label: string;
	targetHandle: string;
};

type EdgeOrderPanelProps = {
	edges: Edge[];
	nodes: Node<ScriptNodeData>[];
	selectedEdge: Edge;
	onDeleteEdge: (edgeId: string) => void;
	onReorder: (edgeIds: string[]) => void;
	onSelectEdge: (edgeId: string) => void;
};

export function EdgeOrderPanel({
	edges,
	nodes,
	selectedEdge,
	onDeleteEdge,
	onReorder,
	onSelectEdge,
}: EdgeOrderPanelProps) {
	const nodesById = new Map(nodes.map((node) => [node.id, node]));
	const sourceNode = nodesById.get(selectedEdge.source);
	const orderedEdges = getOrderedExecutionGroup(edges, selectedEdge.source, selectedEdge.sourceHandle);
	const rows: EdgeOrderRow[] = orderedEdges.map((edge) => ({
		id: edge.id,
		label: getNodeDisplayName(nodesById.get(edge.target)),
		targetHandle: edge.targetHandle ?? "input",
	}));
	const reorder = useReorderController({
		rows,
		onCommit: (nextRows) => onReorder(nextRows.map((row) => row.id)),
	});
	const draggedRow = reorder.drag ? rows.find((row) => row.id === reorder.drag?.draggedId) : null;

	const moveRow = (rowIndex: number, offset: -1 | 1) => {
		const targetIndex = rowIndex + offset;
		if (targetIndex < 0 || targetIndex >= rows.length) {
			return;
		}
		const nextRows = [...rows];
		[nextRows[rowIndex], nextRows[targetIndex]] = [nextRows[targetIndex], nextRows[rowIndex]];
		onReorder(nextRows.map((row) => row.id));
	};

	return (
		<div className="space-y-4 p-4">
			<section>
				<div className="text-xs font-bold uppercase tracking-[0.18em] text-baud-muted">Connection</div>
				<h2 className="mt-2 truncate text-sm font-bold text-white">{getNodeDisplayName(sourceNode)}</h2>
				<p className="mt-1 font-mono text-xs text-baud-muted">Output: {selectedEdge.sourceHandle ?? "out"}</p>
			</section>

			<section className="border-t border-baud-border pt-4">
				<div className="mb-3">
					<h3 className="text-xs font-bold uppercase tracking-[0.18em] text-baud-muted">Execution order</h3>
					<p className="mt-1 text-xs leading-5 text-baud-muted">
						Connected nodes run sequentially from top to bottom. Drag rows or use the arrow buttons to change the order.
					</p>
				</div>

				<ul
					ref={reorder.listRef}
					aria-label={`Execution order for ${getNodeDisplayName(sourceNode)} output ${selectedEdge.sourceHandle ?? "out"}`}
					className="overflow-hidden rounded border border-baud-border bg-baud-soft/60"
				>
					{reorder.entries.map((entry) => {
						if (entry.type === "drop-space") {
							return <li key={entry.id} aria-hidden="true" style={{ height: entry.height }} />;
						}

						const row = entry.row;
						const rowIndex = rows.findIndex((candidate) => candidate.id === row.id);
						const selected = row.id === selectedEdge.id;
						return (
							<li
								key={row.id}
								ref={reorder.registerRow(row.id)}
								className={`flex min-h-12 items-center gap-2 border-b border-baud-border px-2 py-2 last:border-b-0 ${
									selected ? "bg-baud-purple/10" : ""
								}`}
							>
								<span className="flex size-6 shrink-0 items-center justify-center rounded bg-baud-panel font-mono text-xs font-bold text-white">
									{rowIndex + 1}
								</span>
								<Button
									type="button"
									size="icon-sm"
									variant="ghost"
									aria-label={`Drag ${row.label} to reorder`}
									onPointerDown={(event) => reorder.startDrag(row.id, event)}
									className="shrink-0 cursor-grab active:cursor-grabbing"
								>
									<GripVertical size={16} />
								</Button>
								<Button
									type="button"
									size="none"
									variant="ghost"
									onClick={() => onSelectEdge(row.id)}
									className="min-w-0 flex-1 justify-start text-left hover:text-white"
								>
									<span className="min-w-0">
										<span className="block truncate text-sm font-semibold text-white">{row.label}</span>
										<span className="block truncate font-mono text-xs text-baud-muted">Input: {row.targetHandle}</span>
									</span>
								</Button>
								<div className="flex shrink-0 items-center">
									<Button
										type="button"
										size="icon-sm"
										variant="ghost"
										aria-label={`Move ${row.label} earlier`}
										disabled={rowIndex === 0}
										onClick={() => moveRow(rowIndex, -1)}
									>
										<ArrowUp size={14} />
									</Button>
									<Button
										type="button"
										size="icon-sm"
										variant="ghost"
										aria-label={`Move ${row.label} later`}
										disabled={rowIndex === rows.length - 1}
										onClick={() => moveRow(rowIndex, 1)}
									>
										<ArrowDown size={14} />
									</Button>
								</div>
							</li>
						);
					})}
				</ul>
				{rows.length === 1 && (
					<p className="mt-2 text-xs leading-5 text-baud-muted">This output has one connection, so it runs first.</p>
				)}
			</section>

			<Button type="button" variant="destructive" className="w-full" onClick={() => onDeleteEdge(selectedEdge.id)}>
				<Unlink2 size={15} />
				Disconnect selected edge
			</Button>

			{draggedRow && reorder.drag && <FloatingEdgeRow drag={reorder.drag}>{draggedRow.label}</FloatingEdgeRow>}
		</div>
	);
}

function FloatingEdgeRow({ drag, children }: { drag: ActiveReorderDragState; children: ReactNode }) {
	return (
		<div
			className="pointer-events-none fixed z-9999 flex items-center gap-2 rounded border border-baud-purple bg-baud-panel px-3 py-2 opacity-95 shadow-[0_18px_42px_rgba(0,0,0,0.38)]"
			style={{
				left: drag.pointerX - drag.pointerOffsetX,
				minHeight: drag.cardHeight,
				top: drag.pointerY - drag.pointerOffsetY,
				width: drag.cardWidth,
			}}
		>
			<GripVertical size={16} className="text-baud-muted" />
			<span className="truncate text-sm font-semibold text-white">{children}</span>
		</div>
	);
}

function getNodeDisplayName(node: Node<ScriptNodeData> | undefined) {
	if (!node) {
		return "Unknown node";
	}
	const customName = node.data.config.customName;
	return typeof customName === "string" && customName.trim() ? customName.trim() : node.data.label;
}

import type { Edge, Node } from "@xyflow/react";
import type { RiskLevel, ScriptNodeData } from "@/lib/types";

type StatusBarProps = {
	nodes: Node<ScriptNodeData>[];
	edges: Edge[];
	riskLevel: RiskLevel;
};

export function StatusBar({ nodes, edges, riskLevel }: StatusBarProps) {
	return (
		<footer className="flex h-6 shrink-0 items-center justify-between border-t border-baud-border bg-baud-panel px-3 font-mono text-sm text-baud-muted">
			<div className="flex items-center gap-4">
				<span className="flex items-center gap-1 text-baud-green">
					<span className="size-1.5 rounded-full bg-baud-green" />
					ready
				</span>
				<span>{nodes.length} nodes</span>
				<span>{edges.length} edges</span>
				<span>risk {riskLevel}</span>
				<span>format v1</span>
				<span>lang v1</span>
			</div>
			<span>BaudBound Editor 0.1.0</span>
		</footer>
	);
}

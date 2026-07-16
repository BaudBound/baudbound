import type { Edge, Node } from "@xyflow/react";
import type { RiskLevel, ScriptNodeData } from "@/lib/types";
import { EDITOR_CREATED_WITH } from "@/lib/version";

type StatusBarProps = {
	nodes: Node<ScriptNodeData>[];
	edges: Edge[];
	riskLevel: RiskLevel;
};

export function StatusBar({ nodes, edges, riskLevel }: StatusBarProps) {
	return (
		<footer className="flex h-6 shrink-0 items-center justify-between gap-3 overflow-hidden border-t border-baud-border bg-baud-panel px-3 font-mono text-xs text-baud-muted sm:text-sm">
			<div className="flex min-w-0 items-center gap-3 overflow-hidden sm:gap-4">
				<span className="flex items-center gap-1 text-baud-green">
					<span className="size-1.5 rounded-full bg-baud-green" />
					ready
				</span>
				<span>{nodes.length} nodes</span>
				<span>{edges.length} edges</span>
				<span>risk {riskLevel}</span>
				<span className="hidden md:inline">package v1</span>
				<span className="hidden lg:inline">runtime v1</span>
			</div>
			<span className="hidden shrink-0 sm:inline">{EDITOR_CREATED_WITH}</span>
		</footer>
	);
}

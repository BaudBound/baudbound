import type { Edge, Node } from "@xyflow/react";
import type { RiskLevel, ScriptNodeData, TargetRuntime } from "@/lib/types";
import { EDITOR_CREATED_WITH } from "@/lib/version";
import type { VerificationStatus } from "@/utils/verification";

type StatusBarProps = {
	nodes: Node<ScriptNodeData>[];
	edges: Edge[];
	riskLevel: RiskLevel;
	targetRuntime: TargetRuntime;
	verificationStatus: VerificationStatus;
};

export function StatusBar({ nodes, edges, riskLevel, targetRuntime, verificationStatus }: StatusBarProps) {
	const verification = getVerificationPresentation(verificationStatus);

	return (
		<footer className="flex h-6 shrink-0 items-center justify-between gap-3 overflow-hidden border-t border-baud-border bg-baud-panel px-3 font-mono text-xs text-baud-muted sm:text-sm">
			<div className="flex min-w-0 items-center gap-3 overflow-hidden sm:gap-4">
				<span className={`flex items-center gap-1 ${verification.textClassName}`}>
					<span className={`size-1.5 rounded-full ${verification.dotClassName}`} />
					{verification.label}
				</span>
				<span>{targetRuntime}</span>
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

function getVerificationPresentation(status: VerificationStatus) {
	if (status === "verified") {
		return { dotClassName: "bg-baud-green", label: "verified", textClassName: "text-baud-green" };
	}

	if (status === "warning") {
		return { dotClassName: "bg-baud-amber", label: "warnings", textClassName: "text-baud-amber" };
	}

	if (status === "failed") {
		return { dotClassName: "bg-baud-danger", label: "failed", textClassName: "text-baud-danger" };
	}

	return { dotClassName: "bg-baud-muted", label: "not verified", textClassName: "text-baud-muted" };
}

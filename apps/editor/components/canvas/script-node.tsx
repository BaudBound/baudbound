import { Handle, type Node, type NodeProps, Position } from "@xyflow/react";
import { kindAccentClassName } from "@/data/editor/risk";
import type { JsonValue, ScriptNodeData } from "@/lib/types";
import { RiskBadge } from "../shell/risk-badge";

type ScriptFlowNode = Node<ScriptNodeData, "scriptNode">;

const compactHeaderHeight = 44;
const namedHeaderHeight = 58;
const outputHandleSpacing = 30;
const baseBodyHeight = 62;

export function ScriptNode({ data, selected }: NodeProps<ScriptFlowNode>) {
	const customName = typeof data.config.customName === "string" ? data.config.customName.trim() : "";
	const configEntries = Object.entries(data.config).filter(([key]) => key !== "customName");
	const headerHeight = customName ? namedHeaderHeight : compactHeaderHeight;
	const bodyMinHeight = getBodyMinHeight(data.outputs.length);

	return (
		<div
			className={`relative w-64 rounded border bg-baud-node shadow-[0_14px_40px_rgba(0,0,0,0.24)] ${
				selected ? "border-baud-red ring-2 ring-baud-red/35" : "border-baud-border"
			}`}
		>
			<div
				className="flex items-center gap-2.5 border-b border-baud-border px-4 py-2"
				style={{ minHeight: headerHeight }}
			>
				<span className={`size-2.5 rounded-sm ${kindAccentClassName[data.kind]}`} />
				<div className="min-w-0 flex-1">
					<div className="truncate text-base leading-5 font-bold text-white">{data.label}</div>
					{customName && <div className="mt-0.5 truncate font-mono text-xs leading-4 text-baud-text">{customName}</div>}
				</div>
				<div className="shrink-0 self-start pt-0.5 font-mono text-xs text-baud-muted uppercase">{data.kind}</div>
			</div>

			<div className="space-y-1.5 px-4 py-3 pr-20" style={{ minHeight: bodyMinHeight }}>
				{configEntries.length === 0 ? (
					<div className="font-mono text-sm text-baud-muted">No configuration</div>
				) : (
					configEntries.slice(0, 2).map(([key, value]) => (
						<div key={key} className="grid grid-cols-[76px_minmax(0,1fr)] gap-2.5 font-mono text-sm">
							<span className="truncate text-baud-muted">{key}</span>
							<span className="truncate text-baud-text">{formatConfigPreview(value)}</span>
						</div>
					))
				)}
			</div>

			<div className="flex items-center justify-between gap-2 border-t border-baud-border px-4 py-2 pr-20">
				<span className="min-w-0 truncate font-mono text-sm text-baud-muted">{data.actionType}</span>
				<RiskBadge risk={data.risk} />
			</div>

			{data.inputs.map((input) => (
				<Handle
					key={input.id}
					type="target"
					id={input.id}
					position={Position.Left}
					className="size-3! border-baud-blue! bg-baud-panel!"
				/>
			))}
			{data.outputs.map((output, index) => {
				const top = getOutputTop(index, data.outputs.length, bodyMinHeight, headerHeight);

				return (
					<div key={output.id}>
						<span
							className="pointer-events-none absolute right-5 max-w-16 -translate-y-1/2 truncate font-mono text-xs text-baud-muted"
							style={{ top }}
						>
							{output.label}
						</span>
						<Handle
							type="source"
							id={output.id}
							position={Position.Right}
							style={{ top }}
							className={getOutputHandleClassName(output.id)}
						/>
					</div>
				);
			})}
		</div>
	);
}

function getBodyMinHeight(totalOutputs: number) {
	return Math.max(baseBodyHeight, totalOutputs * outputHandleSpacing);
}

function getOutputTop(index: number, total: number, bodyHeight: number, headerHeight: number) {
	if (total === 1) {
		return headerHeight + bodyHeight / 2;
	}

	const first = headerHeight + outputHandleSpacing / 2;
	return first + index * outputHandleSpacing;
}

function getOutputHandleClassName(outputId: string) {
	if (outputId === "success") {
		return "!size-3 !border-baud-green !bg-baud-panel";
	}

	if (outputId === "failed") {
		return "!size-3 !border-baud-danger !bg-baud-panel";
	}

	return "!size-3 !border-baud-red !bg-baud-panel";
}

function formatConfigPreview(value: JsonValue) {
	if (Array.isArray(value)) {
		return `${value.length} item${value.length === 1 ? "" : "s"}`;
	}

	if (typeof value === "object" && value !== null) {
		return "configured";
	}

	return String(value);
}

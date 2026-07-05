import type { Node } from "@xyflow/react";
import { ChevronDown } from "lucide-react";
import { CopyTextButton } from "@/components/common/copy-text-button";
import type { ScriptNodeData } from "@/lib/types";

type RuntimeDataPanelProps = {
	selectedNode: Node<ScriptNodeData>;
};

export function RuntimeDataPanel({ selectedNode }: RuntimeDataPanelProps) {
	const runtimeOutputs = selectedNode.data.runtimeOutputs ?? [];

	return (
		<details className="group rounded border border-baud-border bg-baud-soft">
			<summary className="flex cursor-pointer list-none items-center justify-between gap-2 px-3 py-2">
				<span className="text-xs font-bold uppercase tracking-[0.18em] text-baud-muted">Runtime Data</span>
				<ChevronDown size={14} className="text-baud-muted transition-transform group-open:rotate-180" />
			</summary>
			<div className="border-t border-baud-border p-3">
				{runtimeOutputs.length === 0 ? (
					<div className="text-sm leading-5 text-baud-muted">This node does not produce runtime data.</div>
				) : (
					<div className="space-y-2">
						{runtimeOutputs.map((output) => (
							<div key={output.name} className="border-b border-baud-border pb-2 last:border-b-0 last:pb-0">
								<div className="flex items-center justify-between gap-3">
									<span className="font-mono text-sm text-baud-text">{output.name}</span>
									<span className="font-mono text-sm text-baud-muted">{output.type}</span>
								</div>
								<p className="mt-1 text-sm leading-5 text-baud-muted">{output.description}</p>
								<div className="mt-1 flex items-center gap-2">
									<p className="min-w-0 break-all font-mono text-sm leading-5 text-baud-text">
										{selectedNode.id}.{output.name}
									</p>
									<CopyTextButton text={`${selectedNode.id}.${output.name}`} label="Copy runtime reference" />
								</div>
								{output.fields && (
									<div className="mt-2 space-y-1 rounded border border-baud-border bg-baud-panel/70 p-2">
										{output.fields.map((field) => {
											const reference = `${selectedNode.id}.${output.name}.${field.name}`;

											return (
												<div
													key={field.name}
													className="border-b border-baud-border/70 pb-1.5 last:border-b-0 last:pb-0"
												>
													<div className="flex items-center justify-between gap-2">
														<span className="font-mono text-sm text-baud-text">{field.name}</span>
														<span className="font-mono text-xs text-baud-muted">{field.type}</span>
													</div>
													<p className="mt-0.5 text-xs leading-4 text-baud-muted">{field.description}</p>
													<div className="mt-0.5 flex items-center gap-2">
														<p className="min-w-0 break-all font-mono text-xs leading-4 text-baud-text">{reference}</p>
														<CopyTextButton text={reference} label={`Copy ${field.name} reference`} />
													</div>
												</div>
											);
										})}
									</div>
								)}
							</div>
						))}
					</div>
				)}
			</div>
		</details>
	);
}

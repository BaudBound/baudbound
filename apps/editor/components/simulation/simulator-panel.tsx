import { Info, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { OptionCombobox } from "@/components/ui/option-combobox";
import { AddOverrideControl } from "./add-override-control";
import { normalizeSimulationSpeed, outcomeOptions, speedOptions } from "./simulator-options";
import type { SimulatorPanelProps } from "./simulator-panel-types";
import { TriggerInputCard } from "./trigger-input-card";

export function SimulatorPanel({
	nodes,
	overrides,
	settings,
	status,
	onAddOverride,
	onRemoveOverride,
	onSettingsChange,
	onTriggerSimulation,
	onUpdateOverride,
}: SimulatorPanelProps) {
	const overrideNodeIds = new Set(overrides.map((override) => override.nodeId));
	const availableNodes = nodes.filter((node) => !overrideNodeIds.has(node.id));
	const selectedDefaultNode = availableNodes[0]?.id ?? "";
	const triggerNodes = nodes.filter((node) => node.data.kind === "trigger");
	const nodeOptions = availableNodes.map((node) => ({
		label: `${node.data.label} (${node.id})`,
		value: node.id,
	}));

	return (
		<div className="space-y-4 p-4">
			<div className="rounded border border-baud-border bg-baud-soft p-3">
				<div className="flex items-start gap-3">
					<Info className="mt-0.5 text-baud-muted" size={16} />
					<div>
						<div className="text-sm font-semibold text-white">Simulator</div>
						<p className="mt-1 text-xs leading-5 text-baud-muted">
							Run the verified graph through the editor simulator. Browser-capable actions run live, runner-only actions
							are described in the simulation log, and overrides force selected nodes to use their success or failed
							path.
						</p>
					</div>
				</div>
			</div>

			<div className="rounded border border-baud-border bg-baud-soft/60 p-3">
				<div className="flex items-start justify-between gap-3">
					<div>
						<h3 className="text-xs font-bold tracking-[0.18em] text-baud-muted uppercase">Run State</h3>
						<p className="mt-1 font-mono text-sm text-baud-text">{status}</p>
					</div>
				</div>
			</div>

			<section className="space-y-2 rounded border border-baud-border bg-baud-soft/60 p-3">
				<div>
					<h3 className="text-xs font-bold tracking-[0.18em] text-baud-muted uppercase">Simulation Speed</h3>
					<p className="mt-1 text-xs leading-5 text-baud-muted">Controls how quickly simulator steps are played.</p>
				</div>
				<OptionCombobox
					ariaLabel="Simulation speed"
					value={settings.speed}
					options={speedOptions}
					onChange={(value) => onSettingsChange({ ...settings, speed: normalizeSimulationSpeed(value) })}
				/>
			</section>

			<section className="space-y-3 rounded border border-baud-border bg-baud-soft/60 p-3">
				<div>
					<h3 className="text-xs font-bold tracking-[0.18em] text-baud-muted uppercase">Trigger Input</h3>
					<p className="mt-1 text-xs leading-5 text-baud-muted">
						Every trigger node is available here. Manual and event triggers can be fired while the simulation is
						waiting. Schedule triggers run automatically from their configured interval.
					</p>
				</div>
				{triggerNodes.length === 0 ? (
					<div className="rounded border border-baud-border bg-baud-soft p-3 text-sm leading-5 text-baud-muted">
						No trigger nodes are available.
					</div>
				) : (
					<div className="space-y-2">
						{triggerNodes.map((triggerNode) => (
							<TriggerInputCard
								key={triggerNode.id}
								status={status}
								triggerNode={triggerNode}
								onTrigger={onTriggerSimulation}
							/>
						))}
					</div>
				)}
			</section>

			<section className="space-y-3">
				<div>
					<h3 className="text-xs font-bold tracking-[0.18em] text-baud-muted uppercase">Node Overrides</h3>
					<p className="mt-1 text-xs leading-5 text-baud-muted">
						Force a node to take a success or failed result during simulation.
					</p>
				</div>

				{availableNodes.length > 0 && (
					<AddOverrideControl
						nodeOptions={nodeOptions}
						defaultNodeId={selectedDefaultNode}
						onAddOverride={onAddOverride}
					/>
				)}

				{overrides.length === 0 ? (
					<div className="rounded border border-baud-border bg-baud-soft p-3 text-sm leading-5 text-baud-muted">
						No overrides configured. Nodes use their normal simulated result.
					</div>
				) : (
					<div className="space-y-2">
						{overrides.map((override) => {
							const node = nodes.find((currentNode) => currentNode.id === override.nodeId);

							return (
								<div key={override.nodeId} className="rounded border border-baud-border bg-baud-panel p-3">
									<div className="mb-2 flex items-start justify-between gap-3">
										<div className="min-w-0">
											<div className="truncate text-sm font-bold text-baud-text">
												{node?.data.label ?? "Missing node"}
											</div>
											<div className="mt-1 break-all font-mono text-xs text-baud-muted">{override.nodeId}</div>
										</div>
										<Button
											type="button"
											onClick={() => onRemoveOverride(override.nodeId)}
											aria-label="Remove override"
											size="xsIcon"
											variant="destructive"
										>
											<X size={13} />
										</Button>
									</div>
									<OptionCombobox
										ariaLabel="Override result"
										value={override.outcome}
										options={outcomeOptions}
										onChange={(value) => onUpdateOverride(override.nodeId, value === "failed" ? "failed" : "success")}
									/>
								</div>
							);
						})}
					</div>
				)}
			</section>
		</div>
	);
}

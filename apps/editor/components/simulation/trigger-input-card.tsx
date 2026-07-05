import type { Node } from "@xyflow/react";
import { Play } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { OptionCombobox } from "@/components/ui/option-combobox";
import { Textarea } from "@/components/ui/textarea";
import { fileWatchEventOptions, httpMethodOptions } from "@/data/nodes/definitions/options";
import type { HeaderRow } from "@/data/nodes/definitions/rows";
import type { ScriptNodeData, SimulationRunStatus, SimulationTriggerPayload } from "@/lib/types";
import { KeyValueRowsEditor } from "./key-value-rows-editor";
import { ScheduleTriggerStatus } from "./schedule-trigger-status";
import { createDefaultTriggerPayload, createDefaultWebhookHeaders, createTriggerPayload } from "./trigger-payload";

type TriggerInputCardProps = {
	status: SimulationRunStatus;
	triggerNode: Node<ScriptNodeData>;
	onTrigger: (triggerNodeId: string, payload: SimulationTriggerPayload) => void;
};

export function TriggerInputCard({ status, triggerNode, onTrigger }: TriggerInputCardProps) {
	const [payload, setPayload] = useState<SimulationTriggerPayload>(() => createDefaultTriggerPayload(triggerNode));
	const [webhookHeaders, setWebhookHeaders] = useState<HeaderRow[]>(() => createDefaultWebhookHeaders());
	const [webhookQuery, setWebhookQuery] = useState<HeaderRow[]>(() => []);
	const canTrigger = status === "waiting";

	useEffect(() => {
		setPayload(createDefaultTriggerPayload(triggerNode));
		setWebhookHeaders(createDefaultWebhookHeaders());
		setWebhookQuery([]);
	}, [triggerNode]);

	const updatePayload = (key: keyof SimulationTriggerPayload, value: string) => {
		setPayload((currentPayload) => ({ ...currentPayload, [key]: value }));
	};
	const handleTrigger = () => {
		onTrigger(triggerNode.id, createTriggerPayload(triggerNode, payload, webhookHeaders, webhookQuery));
	};

	return (
		<div className="space-y-2 rounded border border-baud-border bg-baud-panel p-3">
			<div className="flex items-start justify-between gap-3">
				<div className="min-w-0">
					<div className="truncate text-sm font-bold text-baud-text">{triggerNode.data.label}</div>
					<div className="mt-1 break-all font-mono text-xs text-baud-muted">{triggerNode.id}</div>
				</div>
				{triggerNode.data.actionType !== "trigger.schedule" && (
					<Button type="button" disabled={!canTrigger} onClick={handleTrigger} size="sm" variant="toolbarActive">
						<Play size={13} />
						Trigger
					</Button>
				)}
			</div>
			{triggerNode.data.actionType === "trigger.schedule" && (
				<ScheduleTriggerStatus status={status} triggerNode={triggerNode} onTrigger={onTrigger} />
			)}
			{triggerNode.data.actionType === "trigger.serial_input" && (
				<InputField label="Serial data" value={payload.data ?? ""} onChange={(value) => updatePayload("data", value)} />
			)}
			{triggerNode.data.actionType === "trigger.file_watch" && (
				<div className="grid gap-2 sm:grid-cols-2">
					<InputField label="Path" value={payload.path ?? ""} onChange={(value) => updatePayload("path", value)} />
					<div>
						<span className="mb-1 block font-mono text-xs text-baud-muted">Event</span>
						<OptionCombobox
							ariaLabel="File watcher event"
							value={payload.event ?? "modified"}
							options={fileWatchEventOptions}
							onChange={(value) => updatePayload("event", value)}
						/>
					</div>
				</div>
			)}
			{triggerNode.data.actionType === "trigger.webhook" && (
				<div className="space-y-2">
					<div className="grid gap-2 sm:grid-cols-2">
						<div>
							<span className="mb-1 block font-mono text-xs text-baud-muted">Method</span>
							<OptionCombobox
								ariaLabel="Webhook method"
								value={payload.method ?? "POST"}
								options={httpMethodOptions}
								onChange={(value) => updatePayload("method", value)}
							/>
						</div>
						<InputField label="Path" value={payload.path ?? ""} onChange={(value) => updatePayload("path", value)} />
					</div>
					<KeyValueRowsEditor
						addLabel="Add query"
						emptyText="No query parameters"
						namePlaceholder="Parameter"
						rows={webhookQuery}
						title="Query"
						valuePlaceholder="Value"
						onChange={setWebhookQuery}
					/>
					<KeyValueRowsEditor
						addLabel="Add header"
						emptyText="No headers"
						namePlaceholder="Header"
						rows={webhookHeaders}
						title="Headers"
						valuePlaceholder="Value"
						onChange={setWebhookHeaders}
					/>
					<div>
						<span className="mb-1 block font-mono text-xs text-baud-muted">Body</span>
						<Textarea value={payload.body ?? ""} onChange={(event) => updatePayload("body", event.target.value)} />
					</div>
				</div>
			)}
			{triggerNode.data.actionType === "trigger.websocket" && (
				<div className="space-y-2">
					<div className="grid gap-2 sm:grid-cols-2">
						<InputField label="Path" value={payload.path ?? ""} onChange={(value) => updatePayload("path", value)} />
						<InputField
							label="Connection id"
							value={payload.connectionId ?? ""}
							onChange={(value) => updatePayload("connectionId", value)}
						/>
					</div>
					<InputField
						label="Remote address"
						value={payload.remoteAddress ?? ""}
						onChange={(value) => updatePayload("remoteAddress", value)}
					/>
					<KeyValueRowsEditor
						addLabel="Add query"
						emptyText="No query parameters"
						namePlaceholder="Parameter"
						rows={webhookQuery}
						title="Query"
						valuePlaceholder="Value"
						onChange={setWebhookQuery}
					/>
					<KeyValueRowsEditor
						addLabel="Add header"
						emptyText="No headers"
						namePlaceholder="Header"
						rows={webhookHeaders}
						title="Headers"
						valuePlaceholder="Value"
						onChange={setWebhookHeaders}
					/>
					<div>
						<span className="mb-1 block font-mono text-xs text-baud-muted">Message</span>
						<Textarea
							value={payload.message ?? ""}
							onChange={(event) => updatePayload("message", event.target.value)}
						/>
					</div>
				</div>
			)}
			{triggerNode.data.actionType === "trigger.hotkey" && <TriggerConfigSummary value="Uses the configured hotkey." />}
			{triggerNode.data.actionType === "trigger.startup" && (
				<InputField
					label="Startup reason"
					value={payload.reason ?? ""}
					onChange={(value) => updatePayload("reason", value)}
				/>
			)}
			{triggerNode.data.actionType === "trigger.process_started" && (
				<div className="space-y-2">
					<div className="grid gap-2 sm:grid-cols-2">
						<InputField
							label="Process name"
							value={payload.processName ?? ""}
							onChange={(value) => updatePayload("processName", value)}
						/>
						<InputField
							label="Process id"
							value={payload.processId ?? ""}
							onChange={(value) => updatePayload("processId", value)}
						/>
					</div>
					<InputField
						label="Executable path"
						value={payload.executablePath ?? ""}
						onChange={(value) => updatePayload("executablePath", value)}
					/>
					<InputField
						label="Window title"
						value={payload.windowTitle ?? ""}
						onChange={(value) => updatePayload("windowTitle", value)}
					/>
				</div>
			)}
			{triggerNode.data.actionType === "trigger.manual" && (
				<div className="rounded border border-baud-border bg-baud-soft px-3 py-2 text-xs leading-4 text-baud-muted">
					Manual triggers do not require input. Press Trigger to start this branch.
				</div>
			)}
		</div>
	);
}

function InputField({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) {
	return (
		<div>
			<span className="mb-1 block font-mono text-xs text-baud-muted">{label}</span>
			<Input value={value} onChange={(event) => onChange(event.target.value)} />
		</div>
	);
}

function TriggerConfigSummary({ value }: { value: string }) {
	return (
		<div className="rounded border border-baud-border bg-baud-soft px-3 py-2 text-xs leading-4 text-baud-muted">
			{value}
		</div>
	);
}

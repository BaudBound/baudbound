import { ChevronDown, Trash2 } from "lucide-react";
import type { DependencyList, ReactNode } from "react";
import { useEffect, useRef } from "react";
import { Button } from "@/components/ui/button";
import { logLevelClassName } from "@/data/editor/output-console";
import type { EditorVariable } from "@/data/project/variables";
import type { LogEntry, SimulationTraceEntry } from "@/lib/types";

type OutputConsoleProps = {
	activeTab: BottomPanelTab;
	follow: BottomPanelFollowState;
	logs: LogEntry[];
	open: boolean;
	systemLogs: LogEntry[];
	simulationLogs: SimulationTraceEntry[];
	variables: EditorVariable[];
	height: number;
	onClearTab: (tab: ClearableBottomPanelTab) => void;
	onFollowChange: (tab: ClearableBottomPanelTab, enabled: boolean) => void;
	onTabChange: (tab: BottomPanelTab) => void;
	onToggle: () => void;
};

export type BottomPanelTab = "system" | "output" | "simulation" | "variables";
export type ClearableBottomPanelTab = Exclude<BottomPanelTab, "variables">;
export type BottomPanelFollowState = Record<ClearableBottomPanelTab, boolean>;

const bottomPanelTabs: Array<{ id: BottomPanelTab; label: string }> = [
	{ id: "system", label: "System" },
	{ id: "output", label: "Output" },
	{ id: "simulation", label: "Simulation" },
	{ id: "variables", label: "Variables" },
];
const MAX_DISPLAY_VALUE_LENGTH = 4000;

export function OutputConsole({
	activeTab,
	follow,
	logs,
	open,
	systemLogs,
	simulationLogs,
	variables,
	height,
	onClearTab,
	onFollowChange,
	onTabChange,
	onToggle,
}: OutputConsoleProps) {
	const handleTabClick = (tab: BottomPanelTab) => {
		onTabChange(tab);
		if (!open) {
			onToggle();
		}
	};

	return (
		<section className="shrink-0 border-t border-baud-border bg-baud-panel" style={{ height: open ? height : 36 }}>
			<div className="flex h-9 items-center justify-between border-b border-baud-border">
				<div className="flex h-full min-w-0 overflow-x-auto">
					{bottomPanelTabs.map((tab) => (
						<Button
							key={tab.id}
							type="button"
							onClick={() => handleTabClick(tab.id)}
							className={`h-full shrink-0 rounded-none border-b-2 px-4 text-xs font-bold tracking-[0.16em] uppercase ${
								activeTab === tab.id ? "border-baud-red text-baud-text" : "border-transparent text-baud-muted"
							}`}
							size="none"
							variant="tab"
						>
							{tab.label}
						</Button>
					))}
				</div>
				<Button
					type="button"
					onClick={onToggle}
					aria-label={open ? "Collapse bottom panel" : "Expand bottom panel"}
					className="h-full w-10 shrink-0 rounded-none"
					size="none"
					variant="ghost"
				>
					<ChevronDown size={14} className={open ? "" : "-rotate-90"} />
				</Button>
			</div>
			{open && (
				<div className="h-[calc(100%-36px)] overflow-hidden">
					{activeTab === "system" && (
						<LogPanel
							empty={systemLogs.length === 0}
							emptyText="System messages will appear here."
							follow={follow.system}
							footerLabel="System log options"
							onClear={() => onClearTab("system")}
							onFollowChange={(enabled) => onFollowChange("system", enabled)}
						>
							<SystemTab logs={systemLogs} />
						</LogPanel>
					)}
					{activeTab === "output" && (
						<LogPanel
							empty={logs.length === 0}
							emptyText="Script output will appear here."
							follow={follow.output}
							footerLabel="Output log options"
							onClear={() => onClearTab("output")}
							onFollowChange={(enabled) => onFollowChange("output", enabled)}
						>
							<OutputTab logs={logs} />
						</LogPanel>
					)}
					{activeTab === "simulation" && (
						<LogPanel
							empty={simulationLogs.length === 0}
							emptyText="Simulation trace output will appear here."
							follow={follow.simulation}
							footerLabel="Simulation log options"
							onClear={() => onClearTab("simulation")}
							onFollowChange={(enabled) => onFollowChange("simulation", enabled)}
						>
							<SimulationTab logs={simulationLogs} />
						</LogPanel>
					)}
					{activeTab === "variables" && <VariablesTab variables={variables} />}
				</div>
			)}
		</section>
	);
}

function LogPanel({
	children,
	empty,
	emptyText,
	follow,
	footerLabel,
	onClear,
	onFollowChange,
}: {
	children: ReactNode;
	empty: boolean;
	emptyText: string;
	follow: boolean;
	footerLabel: string;
	onClear: () => void;
	onFollowChange: (enabled: boolean) => void;
}) {
	const scrollRef = useAutoFollow(follow, [children]);

	return (
		<div className="grid h-full grid-rows-[minmax(0,1fr)_32px]">
			<div ref={scrollRef} className="overflow-y-auto px-4 py-3 select-text" data-selectable-text="true">
				{empty ? (
					<div className="rounded border border-baud-border bg-baud-soft p-3 text-sm leading-5 text-baud-muted">
						{emptyText}
					</div>
				) : (
					children
				)}
			</div>
			<LogFooter label={footerLabel} follow={follow} onClear={onClear} onFollowChange={onFollowChange} />
		</div>
	);
}

function SystemTab({ logs }: { logs: LogEntry[] }) {
	if (logs.length === 0) {
		return null;
	}

	return (
		<div className="font-mono text-sm leading-6">
			{logs.map((log, index) => (
				<LogLine key={`${log.level}-${log.message}-${index}`} log={log} />
			))}
		</div>
	);
}

function SimulationTab({ logs }: { logs: SimulationTraceEntry[] }) {
	if (logs.length === 0) {
		return null;
	}

	return (
		<div className="font-mono text-sm leading-6">
			{logs.map((log, index) => (
				<div key={`${log.level}-${log.message}-${index}`} className={getSimulationMessageClassName(log.level)}>
					<span className="text-baud-green">[Simulation]</span> {stripSimulationPrefix(log.message)}
				</div>
			))}
		</div>
	);
}

function stripSimulationPrefix(message: string) {
	return message.replace(/^\[Simulation\]\s*/, "");
}

function getSimulationMessageClassName(level: SimulationTraceEntry["level"]) {
	if (level === "error") {
		return "whitespace-pre-wrap text-baud-danger";
	}

	if (level === "warn") {
		return "whitespace-pre-wrap text-baud-amber";
	}

	return "whitespace-pre-wrap text-baud-muted";
}

function OutputTab({ logs }: { logs: LogEntry[] }) {
	if (logs.length === 0) {
		return null;
	}

	return (
		<div className="font-mono text-sm leading-6">
			{logs.map((log, index) => (
				<LogLine key={`${log.level}-${log.message}-${index}`} log={log} />
			))}
		</div>
	);
}

function LogLine({ log }: { log: LogEntry }) {
	return (
		<div className={`whitespace-pre-wrap ${log.level === "error" ? "text-baud-danger" : "text-baud-muted"}`}>
			<span className={logLevelClassName[log.level]}>[{log.level}]</span> {log.message}
		</div>
	);
}

function VariablesTab({ variables }: { variables: EditorVariable[] }) {
	if (variables.length === 0) {
		return (
			<div className="h-full overflow-y-auto px-4 py-3 select-text" data-selectable-text="true">
				<div className="rounded border border-baud-border bg-baud-soft p-3 text-sm leading-5 text-baud-muted">
					Variables will appear here when the script defines them.
				</div>
			</div>
		);
	}

	return (
		<div className="h-full overflow-y-auto px-4 py-3 select-text" data-selectable-text="true">
			<div className="overflow-x-auto rounded border border-baud-border bg-baud-soft">
				<div className="min-w-[720px]">
					<div className="grid grid-cols-[minmax(180px,0.8fr)_96px_104px_88px_minmax(220px,1fr)] gap-3 border-b border-baud-border px-3 py-2 text-xs font-bold tracking-[0.12em] text-baud-muted uppercase">
						<div>Name</div>
						<div>Type</div>
						<div>Scope</div>
						<div>Source</div>
						<div>Value</div>
					</div>
					<div className="divide-y divide-baud-border/80">
						{variables.map((variable) => (
							<div
								key={`${variable.source}-${variable.name}`}
								className="grid grid-cols-[minmax(180px,0.8fr)_96px_104px_88px_minmax(220px,1fr)] gap-3 px-3 py-2 font-mono text-sm"
							>
								<div className="min-w-0">
									<div className="break-all text-baud-text">{variable.name}</div>
									<div className="mt-1 break-all text-xs text-baud-muted">{variable.token}</div>
								</div>
								<div className="min-w-0 break-all text-baud-muted">{variable.type}</div>
								<div className="min-w-0 break-all text-baud-muted">{variable.scope}</div>
								<div className="text-baud-muted">{variable.source}</div>
								<pre className="min-w-0 whitespace-pre-wrap break-all text-baud-muted">
									{formatVariableValue(variable.value)}
								</pre>
							</div>
						))}
					</div>
				</div>
			</div>
		</div>
	);
}

function formatVariableValue(value: EditorVariable["value"]) {
	if (value === undefined) {
		return "available at runtime";
	}

	const formatted = typeof value === "string" ? value : JSON.stringify(value, null, 2);

	if (!formatted) {
		return "";
	}

	return formatted.length > MAX_DISPLAY_VALUE_LENGTH
		? `${formatted.slice(0, MAX_DISPLAY_VALUE_LENGTH)}... [truncated]`
		: formatted;
}

function LogFooter({
	follow,
	label,
	onClear,
	onFollowChange,
}: {
	follow: boolean;
	label: string;
	onClear: () => void;
	onFollowChange: (enabled: boolean) => void;
}) {
	return (
		<fieldset className="flex items-center justify-between border-t border-baud-border px-3">
			<legend className="sr-only">{label}</legend>
			<label className="flex items-center gap-2 text-xs text-baud-muted">
				<input
					type="checkbox"
					checked={follow}
					onChange={(event) => onFollowChange(event.target.checked)}
					className="size-3 accent-baud-red"
				/>
				Follow
			</label>
			<Button type="button" onClick={onClear} size="xs" variant="ghost">
				<Trash2 size={12} />
				Clear
			</Button>
		</fieldset>
	);
}

function useAutoFollow(enabled: boolean, deps: DependencyList) {
	const scrollRef = useRef<HTMLDivElement>(null);

	useEffect(() => {
		if (!enabled || !scrollRef.current) {
			return;
		}

		scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
	}, [enabled, ...deps]);

	return scrollRef;
}

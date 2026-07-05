import type { Node } from "@xyflow/react";
import { Clock } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { ScriptNodeData, SimulationRunStatus, SimulationTriggerPayload } from "@/lib/types";

type ScheduleTriggerStatusProps = {
	status: SimulationRunStatus;
	triggerNode: Node<ScriptNodeData>;
	onTrigger: (triggerNodeId: string, payload: SimulationTriggerPayload) => void;
};

export function ScheduleTriggerStatus({ status, triggerNode, onTrigger }: ScheduleTriggerStatusProps) {
	const intervalMs = getScheduleIntervalMs(triggerNode);
	const active = status === "waiting" || status === "running";
	const statusRef = useRef(status);
	const triggerRef = useRef(onTrigger);
	const [lastRunAt, setLastRunAt] = useState<number | null>(null);
	const [nextRunAt, setNextRunAt] = useState<number | null>(null);
	const [now, setNow] = useState(() => Date.now());

	statusRef.current = status;
	triggerRef.current = onTrigger;

	useEffect(() => {
		if (!active) {
			setNextRunAt(null);
			return;
		}

		const tickId = window.setInterval(() => setNow(Date.now()), 500);
		return () => window.clearInterval(tickId);
	}, [active]);

	useEffect(() => {
		if (!active) {
			return;
		}

		let timeoutId: number | null = null;
		let cancelled = false;
		const scheduleNext = () => {
			if (cancelled) {
				return;
			}

			const nextAt = Date.now() + intervalMs;
			setNextRunAt(nextAt);
			timeoutId = window.setTimeout(() => {
				if (cancelled) {
					return;
				}

				if (statusRef.current === "waiting") {
					const firedAt = Date.now();
					setLastRunAt(firedAt);
					triggerRef.current(triggerNode.id, {});
				}

				scheduleNext();
			}, intervalMs);
		};

		scheduleNext();

		return () => {
			cancelled = true;
			if (timeoutId !== null) {
				window.clearTimeout(timeoutId);
			}
		};
	}, [active, intervalMs, triggerNode.id]);

	return (
		<div className="space-y-2 rounded border border-baud-border bg-baud-soft px-3 py-2 text-xs text-baud-muted">
			<div className="flex items-center gap-2 text-baud-text">
				<Clock size={13} />
				<span className="font-semibold">Automatic schedule</span>
			</div>
			<div className="grid gap-2 font-mono sm:grid-cols-3">
				<ScheduleTime label="Last" value={lastRunAt ? formatTime(lastRunAt) : "not yet"} />
				<ScheduleTime label="Next" value={nextRunAt ? formatTime(nextRunAt) : active ? "scheduling" : "inactive"} />
				<ScheduleTime
					label="In"
					value={nextRunAt && active ? formatDuration(Math.max(0, nextRunAt - now)) : "inactive"}
				/>
			</div>
		</div>
	);
}

function ScheduleTime({ label, value }: { label: string; value: string }) {
	return (
		<div className="rounded border border-baud-border bg-baud-panel px-2 py-1.5">
			<div className="mb-0.5 text-[10px] uppercase tracking-[0.12em] text-baud-muted">{label}</div>
			<div className="text-baud-text">{value}</div>
		</div>
	);
}

function getScheduleIntervalMs(triggerNode: Node<ScriptNodeData>) {
	const every = Number(triggerNode.data.config.every);
	const unit = String(triggerNode.data.config.unit ?? "seconds");
	const safeEvery = Number.isFinite(every) && every > 0 ? every : 1;
	const unitMultiplier = getScheduleUnitMultiplier(unit);

	return Math.max(1000, Math.round(safeEvery * unitMultiplier));
}

function getScheduleUnitMultiplier(unit: string) {
	if (unit === "days") {
		return 24 * 60 * 60 * 1000;
	}

	if (unit === "hours") {
		return 60 * 60 * 1000;
	}

	if (unit === "minutes") {
		return 60 * 1000;
	}

	return 1000;
}

function formatTime(value: number) {
	return new Intl.DateTimeFormat(undefined, {
		hour: "2-digit",
		minute: "2-digit",
		second: "2-digit",
	}).format(new Date(value));
}

function formatDuration(milliseconds: number) {
	const totalSeconds = Math.ceil(milliseconds / 1000);
	const hours = Math.floor(totalSeconds / 3600);
	const minutes = Math.floor((totalSeconds % 3600) / 60);
	const seconds = totalSeconds % 60;

	if (hours > 0) {
		return `${hours}h ${minutes}m ${seconds}s`;
	}

	if (minutes > 0) {
		return `${minutes}m ${seconds}s`;
	}

	return `${seconds}s`;
}

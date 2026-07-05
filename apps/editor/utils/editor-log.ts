import type { LogEntry, SimulationTraceEntry } from "@/lib/types";

const MAX_UI_LOG_MESSAGE_LENGTH = 4000;

export function truncateLogEntry(log: LogEntry): LogEntry {
	return { ...log, message: truncateUiLogMessage(log.message) };
}

export function truncateSimulationTrace(log: SimulationTraceEntry): SimulationTraceEntry {
	return { ...log, message: truncateUiLogMessage(log.message) };
}

function truncateUiLogMessage(message: string) {
	return message.length > MAX_UI_LOG_MESSAGE_LENGTH
		? `${message.slice(0, MAX_UI_LOG_MESSAGE_LENGTH)}... [truncated]`
		: message;
}

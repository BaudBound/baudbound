import type { LogEntry } from "@/lib/types";

export const logLevelClassName: Record<LogEntry["level"], string> = {
	debug: "text-baud-green",
	info: "text-baud-blue",
	warn: "text-baud-amber",
	error: "text-baud-danger",
};

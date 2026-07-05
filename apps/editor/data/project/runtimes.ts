import type { TargetRuntime } from "@/lib/types";

export const targetRuntimes: TargetRuntime[] = [
	"Generic Headless",
	"Linux Headless",
	"Windows Headless",
	"macOS Background",
	"Generic Desktop",
	"Windows Desktop",
	"Linux Desktop",
	"macOS Desktop",
];

export function isDesktopTargetRuntime(targetRuntime: TargetRuntime) {
	return targetRuntime.endsWith("Desktop");
}

import type { ActiveRun, ScriptStatus } from "@/lib/runner-api";

export type ScriptRunControl = "run" | "stop";

export function scriptRunControl(
  script: ScriptStatus,
  activeRuns: ActiveRun[],
): ScriptRunControl {
  return activeRuns.some((run) => run.script_id === script.installed.id)
    ? "stop"
    : "run";
}

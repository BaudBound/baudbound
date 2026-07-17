import type { ActiveRun, ScriptStatus } from "@/lib/runner-api";

export type ScriptRunControl = "run" | "stop" | null;

export function scriptRunControl(
  script: ScriptStatus,
  activeRuns: ActiveRun[],
): ScriptRunControl {
  if (activeRuns.length > 0) return "stop";
  return script.triggers.some((trigger) => trigger.action_type === "trigger.manual")
    ? "run"
    : null;
}

import type { DashboardPayload } from "@/lib/runner-api";

export type RuntimeHealth = {
  detail: string;
  issue: string | null;
  label: string;
  state: "active" | "inactive" | "problem" | "stopping";
};

export function desktopRuntimeHealth(dashboard: DashboardPayload): RuntimeHealth {
  const runner = dashboard.desktop_background;
  if (runner.state === "running") {
    return {
      detail: runner.message || "Desktop background runner is active.",
      issue: null,
      label: "Running",
      state: "active",
    };
  }
  if (runner.state === "stopping") {
    return {
      detail: runner.message || "Desktop background runner is stopping.",
      issue: null,
      label: "Stopping",
      state: "stopping",
    };
  }
  if (runner.state === "failed") {
    return {
      detail: runner.message || "Desktop background runner failed.",
      issue: "Desktop trigger listeners are unavailable until the background runner starts again.",
      label: "Failed",
      state: "problem",
    };
  }

  return {
    detail: runner.message || "Desktop background runner is stopped.",
    issue: "Desktop app trigger listeners are not running.",
    label: "Stopped",
    state: "inactive",
  };
}

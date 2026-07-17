import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { DashboardAction } from "@/lib/app-types";
import type { ActiveRun } from "@/lib/runner-api";
import { DesktopTimeProvider } from "@/lib/time-format";
import { ActiveRunsPanel } from "@/views/active-runs-panel";

const runAction: DashboardAction = async () => true;

function renderPanel(runs: ActiveRun[]) {
  return renderToStaticMarkup(
    <DesktopTimeProvider timeFormat="24-hour">
      <ActiveRunsPanel
        busyActions={new Set()}
        runAction={runAction}
        runs={runs}
        scriptNames={new Map([["script-1", "Example script"]])}
      />
    </DesktopTimeProvider>,
  );
}

describe("ActiveRunsPanel", () => {
  it("shows an explicit idle state", () => {
    const markup = renderPanel([]);

    expect(markup).toContain("Currently running");
    expect(markup).toContain("No scripts are currently running.");
  });

  it("shows live run identity, logs, and a stop command", () => {
    const markup = renderPanel([
      {
        cancellation_requested: false,
        discarded_log_count: 2,
        logs: [
          {
            level: "info",
            message: "Working on the current action",
            node_id: "n-action",
            timestamp_unix_ms: Date.UTC(2026, 6, 17, 8, 15, 0),
          },
        ],
        run_id: "run-1",
        script_id: "script-1",
        started_at_unix_ms: Date.UTC(2026, 6, 17, 8, 14, 0),
        trigger_node_id: "n-manual",
      },
    ]);

    expect(markup).toContain("Example script");
    expect(markup).toContain("run-1");
    expect(markup).toContain("n-manual");
    expect(markup).toContain("Working on the current action");
    expect(markup).toContain("2 older live logs were omitted");
    expect(markup).toContain("Follow output");
    expect(markup).toContain('data-state="checked"');
    expect(markup).toContain("Stop");
  });
});

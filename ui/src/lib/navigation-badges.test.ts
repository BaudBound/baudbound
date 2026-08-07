import { describe, expect, it } from "vitest";

import { doctorStatus } from "@/lib/doctor-status";
import { navigationBadge, systemLogNavigationBadge } from "@/lib/navigation-badges";
import type { DashboardPayload, SystemLogSummary } from "@/lib/runner-api";

describe("navigation badges", () => {
  it("shows only the unread error count when lower-severity logs are also unread", () => {
    const badge = systemLogNavigationBadge(systemLogSummary({ unread: 7, unread_errors: 2, unread_info: 5 }));

    expect(badge).toEqual({
      count: 2,
      title: "Unread system logs: 2 errors, 5 information",
      variant: "destructive",
    });
  });

  it("falls through severity priority without combining warnings with informational logs", () => {
    const badge = systemLogNavigationBadge(
      systemLogSummary({ unread: 6, unread_info: 4, unread_successes: 1, unread_warnings: 1 }),
    );

    expect(badge?.count).toBe(1);
    expect(badge?.variant).toBe("medium");
    expect(badge?.title).toBe("Unread system logs: 1 warning, 4 information, 1 success");
  });

  it("shows only information when information and success are unread", () => {
    expect(systemLogNavigationBadge(systemLogSummary({ unread: 5, unread_info: 3, unread_successes: 2 }))).toMatchObject({
      count: 3,
      variant: "muted",
    });
  });

  it("uses the success treatment when only successes are unread", () => {
    expect(systemLogNavigationBadge(systemLogSummary({ unread: 2, unread_successes: 2 }))).toMatchObject({
      count: 2,
      variant: "good",
    });
  });

  it("uses authoritative script, active run, and Doctor counts", () => {
    const dashboard = dashboardPayload();
    const doctor = doctorStatus(dashboard);

    expect(navigationBadge("scripts", dashboard, systemLogSummary())).toMatchObject({ count: 2, variant: "medium" });
    expect(navigationBadge("runs", dashboard, systemLogSummary())).toMatchObject({ count: 1, variant: "good" });
    expect(doctor.warningCount).toBe(3);
    expect(navigationBadge("diagnostics", dashboard, systemLogSummary())).toMatchObject({
      count: 3,
      variant: "medium",
    });
  });

  it("only asks to review enabled scripts once something is installed", () => {
    // An empty runner has nothing to enable, so that is idle rather than a
    // warning. Deleting the last script used to leave the card asking for a
    // review of something that no longer existed.
    const empty = dashboardPayload();
    expect(doctorStatus(empty).states.enabledScripts).toBe("idle");

    const installedButDisabled = dashboardPayload();
    installedButDisabled.runner.total_script_count = 2;
    expect(doctorStatus(installedButDisabled).states.enabledScripts).toBe("warn");

    const enabled = dashboardPayload();
    enabled.runner.total_script_count = 2;
    enabled.runner.enabled_script_count = 1;
    expect(doctorStatus(enabled).states.enabledScripts).toBe("ok");
  });
});

function systemLogSummary(patch: Partial<SystemLogSummary> = {}): SystemLogSummary {
  return {
    total: 0,
    unread: 0,
    unread_errors: 0,
    unread_info: 0,
    unread_successes: 0,
    unread_warnings: 0,
    ...patch,
  };
}

function dashboardPayload(): DashboardPayload {
  return {
    active_runs: [
      {
        cancellation_requested: false,
        discarded_log_count: 0,
        logs: [],
        run_id: "run-1",
        script_id: "script-1",
        started_at_unix_ms: 1,
        trigger_node_id: "trigger-1",
      },
    ],
    desktop_background: { running: false },
    launch_at_login_desired: true,
    launch_at_login_registered: false,
    run_statistics: { total: 0 },
    runner: {
      enabled_script_count: 0,
      problem_count: 2,
      scripts: [],
      supported_target_runtimes: [],
      total_script_count: 0,
    },
    serial_devices: [],
  } as unknown as DashboardPayload;
}

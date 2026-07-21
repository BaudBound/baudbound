import { describe, expect, it } from "vitest";

import type { DashboardPayload } from "@/lib/runner-api";
import { serialServiceIsLive } from "@/views/diagnostics/serial-reader-status";

type ServiceSnapshot = Pick<DashboardPayload, "service_health" | "service_status">;

function serviceSnapshot(state: string, stale = false): ServiceSnapshot {
  return {
    service_health: {
      health: stale ? "stale" : state,
      ok: !stale,
      reason: "test",
      stale,
    },
    service_status: { state } as DashboardPayload["service_status"],
  };
}

describe("serial reader diagnostics", () => {
  it("uses reader details only while the service is currently running", () => {
    expect(serialServiceIsLive(serviceSnapshot("running"))).toBe(true);
    expect(serialServiceIsLive(serviceSnapshot("stopped"))).toBe(false);
  });

  it("rejects stale running status left by a dead process", () => {
    expect(serialServiceIsLive(serviceSnapshot("running", true))).toBe(false);
  });

  it("rejects a missing service status", () => {
    const snapshot = serviceSnapshot("missing");
    snapshot.service_status = null;

    expect(serialServiceIsLive(snapshot)).toBe(false);
  });
});

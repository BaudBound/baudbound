import { describe, expect, it } from "vitest";

import {
  applyActiveRunEvent,
  mergeActiveRunState,
  type ActiveRunState,
} from "@/lib/active-run-events";
import type { ActiveRun, ActiveRunEvent } from "@/lib/runner-api";

function run(runId: string, startedAt = 1): ActiveRun {
  return {
    cancellation_requested: false,
    discarded_log_count: 0,
    logs: [],
    run_id: runId,
    script_id: "script-1",
    started_at_unix_ms: startedAt,
    trigger_node_id: "n-trigger",
  };
}

function apply(state: ActiveRunState, event: ActiveRunEvent) {
  return applyActiveRunEvent(state, event);
}

describe("active run events", () => {
  it("applies the complete lifecycle in revision order", () => {
    let state: ActiveRunState = { revision: 0, runs: [] };
    state = apply(state, { kind: "started", revision: 1, run: run("run-1") });
    state = apply(state, {
      kind: "log_emitted",
      discarded_log_count: 0,
      log: {
        level: "info",
        message: "running",
        node_id: "n-log",
        timestamp_unix_ms: 2,
      },
      revision: 2,
      run_id: "run-1",
    });
    state = apply(state, {
      kind: "cancellation_requested",
      revision: 3,
      run_id: "run-1",
    });

    expect(state.runs[0].logs[0].message).toBe("running");
    expect(state.runs[0].cancellation_requested).toBe(true);

    state = apply(state, { kind: "finished", revision: 4, run_id: "run-1" });
    expect(state).toEqual({ revision: 4, runs: [] });
  });

  it("ignores stale events and stale dashboard snapshots", () => {
    const current = { revision: 4, runs: [run("new")] };
    const staleEvent = apply(current, {
      kind: "started",
      revision: 3,
      run: run("old"),
    });
    const staleSnapshot = mergeActiveRunState(current, {
      revision: 2,
      runs: [run("old")],
    });

    expect(staleEvent).toBe(current);
    expect(staleSnapshot).toBe(current);
  });

  it("orders concurrent starts and keeps unknown log deltas recoverable", () => {
    let state: ActiveRunState = { revision: 0, runs: [] };
    state = apply(state, { kind: "started", revision: 1, run: run("later", 2) });
    state = apply(state, { kind: "started", revision: 2, run: run("first", 1) });
    const unknownLog = apply(state, {
      kind: "log_emitted",
      discarded_log_count: 0,
      log: { level: "info", message: "unknown", node_id: null, timestamp_unix_ms: 3 },
      revision: 3,
      run_id: "missing",
    });

    expect(state.runs.map((entry) => entry.run_id)).toEqual(["first", "later"]);
    expect(unknownLog).toBe(state);
  });
});

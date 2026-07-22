import { describe, expect, it } from "vitest";

import type { TriggerMonitorEvent } from "@/lib/runner-api";
import {
  appendTriggerMonitorEvents,
  triggerMonitorEventLimit,
  triggerMonitorEventMatches,
} from "@/lib/trigger-monitor-events";

describe("trigger monitor events", () => {
  it("retains only the latest events in sequence order", () => {
    const events = Array.from(
      { length: triggerMonitorEventLimit + 2 },
      (_, index) => event(index + 1),
    ).reverse();
    const result = appendTriggerMonitorEvents([], events);
    expect(result).toHaveLength(triggerMonitorEventLimit);
    expect(result[0]?.sequence).toBe(3);
    expect(result.at(-1)?.sequence).toBe(triggerMonitorEventLimit + 2);
  });

  it("filters exact trigger types and searches payload data", () => {
    const monitored = event(1);
    expect(
      triggerMonitorEventMatches(
        monitored,
        "serial value",
        "trigger.serial_input",
        "queued",
        "Inventory",
      ),
    ).toBe(true);
    expect(
      triggerMonitorEventMatches(
        monitored,
        "",
        "trigger.webhook",
        "all",
        "Inventory",
      ),
    ).toBe(false);
  });
});

function event(sequence: number): TriggerMonitorEvent {
  return {
    action_type: "trigger.serial_input",
    error: null,
    node_id: "serial-1",
    omitted_event_count: 0,
    payload_bytes: 23,
    payload_json: '{"data":"serial value"}',
    payload_truncated: false,
    script_id: "script-1",
    sequence,
    session_id: 1,
    source: "listener",
    status: "queued",
    timestamp_unix_ms: 1,
  };
}

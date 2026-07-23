import { describe, expect, it } from "vitest";

import type { ActiveRun, ScriptStatus } from "@/lib/runner-api";
import { scriptRunControl } from "@/lib/script-run-control";

function scriptWithTriggers(actionTypes: string[]) {
  return {
    installed: { id: "script-1" },
    triggers: actionTypes.map((action_type, index) => ({
      action_type,
      device_id: null,
      node_id: `n-${index}`,
      runner_type: "desktop",
      target: null,
    })),
  } as ScriptStatus;
}

const activeRun = {
  script_id: "script-1",
  trigger_node_id: "n-0",
} as ActiveRun;

describe("scriptRunControl", () => {
  it("shows Run for every idle script", () => {
    expect(scriptRunControl(scriptWithTriggers(["trigger.manual"]), [])).toBe("run");
    expect(scriptRunControl(scriptWithTriggers(["trigger.schedule"]), [])).toBe("run");
  });

  it("shows Stop for a scheduled execution", () => {
    expect(
      scriptRunControl(scriptWithTriggers(["trigger.schedule"]), [activeRun]),
    ).toBe("stop");
  });

  it("shows Stop while a Manual trigger execution is active", () => {
    expect(scriptRunControl(scriptWithTriggers(["trigger.manual"]), [activeRun])).toBe(
      "stop",
    );
  });

  it("ignores active runs owned by another script", () => {
    expect(
      scriptRunControl(scriptWithTriggers(["trigger.manual"]), [
        { ...activeRun, script_id: "script-2" },
      ]),
    ).toBe("run");
  });
});

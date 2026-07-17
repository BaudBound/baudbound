import { describe, expect, it } from "vitest";

import type { ActiveRun, ScriptStatus } from "@/lib/runner-api";
import { scriptRunControl } from "@/lib/script-run-control";

function scriptWithTriggers(actionTypes: string[]) {
  return {
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
} as ActiveRun;

describe("scriptRunControl", () => {
  it("shows Run only for scripts with a Manual trigger", () => {
    expect(scriptRunControl(scriptWithTriggers(["trigger.manual"]), [])).toBe("run");
    expect(scriptRunControl(scriptWithTriggers(["trigger.schedule"]), [])).toBeNull();
  });

  it("shows Stop whenever the script has an active execution", () => {
    expect(
      scriptRunControl(scriptWithTriggers(["trigger.schedule"]), [activeRun]),
    ).toBe("stop");
  });
});

import { describe, expect, it } from "vitest";

import type { ScriptStatus } from "@/lib/runner-api";
import { scriptProblems } from "@/lib/script-problems";

describe("scriptProblems", () => {
  it("presents one actionable root problem when an installed package cannot load", () => {
    const packageError =
      'program.json does not match the runner schema: /entry/program/steps/1: {"large":"payload"} is invalid';
    const problems = scriptProblems(
      scriptStatus({
        approval_status: { state: "package_unavailable" },
        package_error: packageError,
      }),
    );

    expect(problems).toEqual([
      {
        advancedDetail: packageError,
        detail:
          "The runner could not validate or prepare this installed package, so the script cannot run. Correct the package and reinstall it.",
        id: "package-error",
        severity: "error",
        title: "Package cannot be loaded",
      },
    ]);
  });

  it("continues to report independent approval and trigger problems for loadable packages", () => {
    const problems = scriptProblems(scriptStatus());

    expect(problems.map((problem) => problem.id)).toEqual([
      "approval-missing",
      "no-triggers",
    ]);
  });
});

function scriptStatus(
  patch: Partial<ScriptStatus> = {},
): ScriptStatus {
  return {
    approval_status: { state: "missing" },
    blacklist: { entries: [], severity: null },
    declared_permissions: [],
    installed: {
      asset_count: 0,
      enabled: true,
      id: "script-1",
      imported_at_unix: 1,
      name: "Script One",
      package_file_name: "script-one.bbs",
      package_format_version: 2,
      package_hash: "hash",
      package_path: "scripts/script-one.bbs",
      risk_level: "medium",
      script_language_version: 2,
      target_runtime: "Windows Desktop",
    },
    metadata: null,
    package_error: null,
    package_hash_status: { state: "valid" },
    triggers: [],
    ...patch,
  };
}

import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { ScriptStatus } from "@/lib/runner-api";
import { ScriptProblemPanel } from "@/views/script-problem-panel";

describe("ScriptProblemPanel", () => {
  it("keeps package diagnostics collapsed, bounded, and free of unavailable approval controls", () => {
    const rawError =
      'program.json does not match the runner schema: /entry/program/steps/1: {"large":"payload"} is invalid';
    const markup = renderToStaticMarkup(
      <ScriptProblemPanel
        onApproveScript={() => undefined}
        scriptSettings={{}}
        scripts={[invalidScript(rawError)]}
      />,
    );

    expect(markup).toContain("Package cannot be loaded");
    expect(markup).toContain("Advanced details");
    expect(markup).toContain("<details");
    expect(markup).toContain("max-h-64");
    expect(markup).toContain("break-all");
    expect(markup.match(/program\.json does not match/g)).toHaveLength(1);
    expect(markup).not.toContain("Approval required");
    expect(markup).not.toContain("No active triggers");
    expect(markup).not.toContain("Review approval");
  });
});

function invalidScript(packageError: string): ScriptStatus {
  return {
    approval_status: { state: "package_unavailable" },
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
    package_error: packageError,
    package_hash_status: { state: "valid" },
    triggers: [],
  };
}

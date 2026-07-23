import { describe, expect, it } from "vitest";

import { approvalIssueDescription } from "@/lib/status-format";

describe("approval issue descriptions", () => {
  it("describes missing approval without repeating the status label", () => {
    expect(approvalIssueDescription({ state: "missing" })).toBe(
      "This script needs approval.",
    );
  });

  it("does not report an issue for a current approval", () => {
    expect(approvalIssueDescription({ state: "current" })).toBeNull();
  });
});

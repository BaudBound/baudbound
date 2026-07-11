import { describe, expect, it } from "vitest";

import { approvalReviewState } from "@/lib/approval-review";
import type { ApprovalStatus, PackageHashStatus, ScriptStatus } from "@/lib/runner-api";

describe("approvalReviewState", () => {
  it("allows a verified package with missing approval to be approved", () => {
    const review = approvalReviewState(scriptStatus({ state: "missing" }, { state: "valid" }));

    expect(review).toEqual({
      approvalIsCurrent: false,
      approvalIsStored: false,
      approveBlockedReason: null,
      packageIsApprovable: true,
    });
  });

  it("allows a current approval to be revoked without offering another approval", () => {
    const review = approvalReviewState(scriptStatus({ state: "current" }, { state: "valid" }));

    expect(review.approvalIsCurrent).toBe(true);
    expect(review.approvalIsStored).toBe(true);
    expect(review.packageIsApprovable).toBe(true);
  });

  it("allows an outdated approval to be replaced or revoked", () => {
    const review = approvalReviewState(
      scriptStatus(
        {
          approved_package_hash: "old",
          installed_package_hash: "new",
          state: "stale_package_hash",
        },
        { state: "valid" },
      ),
    );

    expect(review.approvalIsCurrent).toBe(false);
    expect(review.approvalIsStored).toBe(true);
    expect(review.packageIsApprovable).toBe(true);
  });

  it("blocks approval when package integrity does not match", () => {
    const review = approvalReviewState(
      scriptStatus(
        { state: "missing" },
        { actual: "tampered", expected: "installed", state: "mismatch" },
      ),
    );

    expect(review.packageIsApprovable).toBe(false);
    expect(review.approveBlockedReason).toContain("package hash is invalid");
  });

  it("blocks approval when the package cannot be loaded", () => {
    const script = scriptStatus({ state: "missing" }, { state: "valid" });
    script.package_error = "manifest is invalid";

    const review = approvalReviewState(script);

    expect(review.packageIsApprovable).toBe(false);
    expect(review.approveBlockedReason).toContain("cannot load");
  });
});

function scriptStatus(
  approval_status: ApprovalStatus,
  package_hash_status: PackageHashStatus,
): ScriptStatus {
  return {
    approval_status,
    declared_permissions: ["run_process"],
    installed: {
      asset_count: 0,
      enabled: true,
      id: "script-1",
      imported_at_unix: 1,
      name: "Script One",
      package_file_name: "script-one.bbs",
      package_format_version: 1,
      package_hash: "installed",
      package_path: "scripts/script-one.bbs",
      risk_level: "high",
      script_language_version: 1,
      target_runtime: "Generic Desktop",
    },
    package_error: null,
    package_hash_status,
    triggers: [],
  };
}

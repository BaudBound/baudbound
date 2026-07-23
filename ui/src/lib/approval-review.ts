import type { ScriptStatus } from "@/lib/runner-api";
import {
  hasStoredApproval,
  isApprovalCurrent,
  isPackageHashValid,
} from "@/lib/status-format";

export type ApprovalReviewState = {
  approvalIsCurrent: boolean;
  approvalIsStored: boolean;
  approveBlockedReason: string | null;
  packageIsApprovable: boolean;
};

export function approvalReviewState(script: ScriptStatus): ApprovalReviewState {
  const packageIsApprovable =
    script.package_error === null && isPackageHashValid(script.package_hash_status);

  return {
    approvalIsCurrent: isApprovalCurrent(script.approval_status),
    approvalIsStored: hasStoredApproval(script.approval_status),
    approveBlockedReason: approvalBlockReason(script),
    packageIsApprovable,
  };
}

function approvalBlockReason(script: ScriptStatus) {
  if (script.package_error) {
    return "This package cannot be approved because the runner cannot load the installed package. Update or remove the script first.";
  }
  if (!isPackageHashValid(script.package_hash_status)) {
    return "This package cannot be approved while its stored package hash is invalid. Update the installed package before approving it.";
  }
  return null;
}

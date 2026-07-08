import type { ApprovalStatus, PackageHashStatus, ScriptStatus } from "@/lib/runner-api";
import { approvalLabel, packageHashLabel } from "@/lib/status-format";

export type ScriptProblemSeverity = "error" | "warning";

export type ScriptProblem = {
  detail: string;
  id: string;
  severity: ScriptProblemSeverity;
  title: string;
};

export function scriptProblems(script: ScriptStatus): ScriptProblem[] {
  const problems: ScriptProblem[] = [];

  if (script.package_error) {
    problems.push({
      detail: script.package_error,
      id: "package-error",
      severity: "error",
      title: "Package cannot be loaded",
    });
  }

  const hashProblem = packageHashProblem(script.package_hash_status);
  if (hashProblem) problems.push(hashProblem);

  const approvalProblem = approvalStatusProblem(script.approval_status);
  if (approvalProblem) problems.push(approvalProblem);

  if (!script.installed.enabled) {
    problems.push({
      detail: "Disabled scripts are installed but ignored by trigger registration and automatic execution.",
      id: "disabled",
      severity: "warning",
      title: "Script is disabled",
    });
  }

  if (script.installed.enabled && script.triggers.length === 0) {
    problems.push({
      detail: "This script is enabled but has no active trigger registrations for the current runner.",
      id: "no-triggers",
      severity: "warning",
      title: "No active triggers",
    });
  }

  return problems;
}

export function hasApprovalProblem(status: ApprovalStatus) {
  return approvalLabel(status) !== "current";
}

export function hasBlockingProblem(script: ScriptStatus) {
  return scriptProblems(script).some((problem) => problem.severity === "error");
}

function packageHashProblem(status: PackageHashStatus): ScriptProblem | null {
  const label = packageHashLabel(status);
  if (label === "valid") return null;

  if (typeof status === "object" && "Mismatch" in status) {
    return {
      detail: `Expected ${status.Mismatch.expected}, but the installed package currently hashes to ${status.Mismatch.actual}.`,
      id: "hash-mismatch",
      severity: "error",
      title: "Package hash mismatch",
    };
  }

  if (typeof status === "object" && "Error" in status) {
    return {
      detail: status.Error,
      id: "hash-error",
      severity: "error",
      title: "Package hash check failed",
    };
  }

  return {
    detail: `Package hash status is ${label}.`,
    id: "hash-unknown",
    severity: "warning",
    title: "Package hash is not verified",
  };
}

function approvalStatusProblem(status: ApprovalStatus): ScriptProblem | null {
  if (!hasApprovalProblem(status)) return null;

  if (typeof status === "object" && "StalePackageHash" in status) {
    return {
      detail: `Approved hash ${status.StalePackageHash.approved_package_hash}, installed hash ${status.StalePackageHash.installed_package_hash}. Review and approve again if this update is expected.`,
      id: "approval-stale-hash",
      severity: "error",
      title: "Approval is stale",
    };
  }

  if (typeof status === "object" && "Error" in status) {
    return {
      detail: status.Error,
      id: "approval-error",
      severity: "error",
      title: "Approval check failed",
    };
  }

  const label = approvalLabel(status);
  const detailByLabel: Record<string, string> = {
    missing: "This script has not been approved on this runner.",
    "packageunavailable": "The installed package is unavailable, so approval cannot be validated.",
    "permissionmismatch": "The package permissions changed after approval. Review and approve again if expected.",
    unknown: "Approval status is unknown. Review the package before running it.",
  };

  return {
    detail: detailByLabel[label] ?? `Approval status is ${label}.`,
    id: `approval-${label}`,
    severity: label === "unknown" ? "warning" : "error",
    title: "Approval required",
  };
}

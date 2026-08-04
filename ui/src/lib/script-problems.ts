import type { ApprovalStatus, PackageHashStatus, ScriptStatus } from "@/lib/runner-api";
import {
  approvalLabel,
  approvalState,
  isApprovalCurrent,
  isPackageHashValid,
  packageHashLabel,
} from "@/lib/status-format";

export type ScriptProblemSeverity = "error" | "warning";

export type ScriptProblem = {
  advancedDetail?: string;
  detail: string;
  id: string;
  severity: ScriptProblemSeverity;
  title: string;
};

export function scriptProblems(script: ScriptStatus): ScriptProblem[] {
  const problems: ScriptProblem[] = [];

  if (script.blacklist.entries.length > 0) {
    const severity = script.blacklist.severity;
    problems.push({
      detail: script.blacklist.entries
        .map((entry) => `${entry.title}: ${entry.reason}`)
        .join(" "),
      id: `blacklist-${severity ?? "advisory"}`,
      severity:
        severity === "medium" || severity === "high" || severity === "critical"
          ? "error"
          : "warning",
      title:
        severity === "high" || severity === "critical"
          ? "Script is quarantined"
          : severity === "medium"
            ? "Distribution is restricted"
            : "Security advisory",
    });
  }

  if (script.package_error) {
    problems.push({
      advancedDetail: script.package_error,
      detail:
        "The runner could not validate or prepare this installed package, so the script cannot run. Correct the package and reinstall it.",
      id: "package-error",
      severity: "error",
      title: "Package cannot be loaded",
    });
  }

  const hashProblem = packageHashProblem(script.package_hash_status);
  if (hashProblem) problems.push(hashProblem);

  if (!script.package_error) {
    const approvalProblem = approvalStatusProblem(script.approval_status);
    if (approvalProblem) problems.push(approvalProblem);
  }

  if (!script.installed.enabled) {
    problems.push({
      detail: "Disabled scripts are installed but ignored by trigger registration and automatic execution.",
      id: "disabled",
      severity: "warning",
      title: "Script is disabled",
    });
  }

  if (
    !script.package_error &&
    script.installed.enabled &&
    script.triggers.length === 0
  ) {
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
  return !isApprovalCurrent(status);
}

export function hasBlockingProblem(script: ScriptStatus) {
  return scriptProblems(script).some((problem) => problem.severity === "error");
}

function packageHashProblem(status: PackageHashStatus): ScriptProblem | null {
  const label = packageHashLabel(status);
  if (isPackageHashValid(status)) return null;

  if (status.state === "mismatch") {
    return {
      detail: `Expected ${status.expected}, but the installed package currently hashes to ${status.actual}.`,
      id: "hash-mismatch",
      severity: "error",
      title: "Package hash mismatch",
    };
  }

  if (status.state === "error") {
    return {
      detail: status.message ?? "Package hash check failed.",
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

  if (status.state === "stale_package_hash") {
    return {
      detail: `Approved hash ${status.approved_package_hash}, installed hash ${status.installed_package_hash}. Review and approve again if this update is expected.`,
      id: "approval-stale-hash",
      severity: "error",
      title: "Approval is stale",
    };
  }

  if (status.state === "error") {
    return {
      detail: status.message ?? "Approval check failed.",
      id: "approval-error",
      severity: "error",
      title: "Approval check failed",
    };
  }

  const state = approvalState(status);
  const detailByState: Record<string, string> = {
    missing: "This script has not been approved on this runner.",
    package_unavailable: "The installed package is unavailable, so approval cannot be validated.",
    permission_mismatch: "The package permissions changed after approval. Review and approve again if expected.",
    unknown: "Approval status is unknown. Review the package before running it.",
  };

  return {
    detail: detailByState[state] ?? `Approval status is ${approvalLabel(status)}.`,
    id: `approval-${state}`,
    severity: state === "unknown" ? "warning" : "error",
    title: "Approval required",
  };
}

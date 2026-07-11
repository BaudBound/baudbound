import type { ApprovalStatus, PackageHashStatus } from "@/lib/runner-api";

export function packageHashLabel(status: PackageHashStatus) {
  return packageHashLabelByState(packageHashState(status));
}

export function approvalLabel(status: ApprovalStatus) {
  return approvalLabelByState(approvalState(status));
}

export function isPackageHashValid(status: PackageHashStatus) {
  return packageHashState(status) === "valid";
}

export function isApprovalCurrent(status: ApprovalStatus) {
  return approvalState(status) === "current";
}

export function hasStoredApproval(status: ApprovalStatus) {
  return ["current", "package_unavailable", "permission_mismatch", "stale_package_hash"].includes(
    approvalState(status),
  );
}

export function approvalVariant(status: ApprovalStatus) {
  if (isApprovalCurrent(status)) return "good";
  if (approvalState(status) === "missing") return "medium";
  return "destructive";
}

export function packageHashState(status: PackageHashStatus) {
  return status.state;
}

export function approvalState(status: ApprovalStatus) {
  return status.state;
}

function packageHashLabelByState(state: string) {
  const labels: Record<string, string> = {
    error: "Check failed",
    mismatch: "Hash changed",
    unknown: "Unknown",
    valid: "Verified",
  };
  return labels[state] ?? titleCase(state.replaceAll("_", " "));
}

function approvalLabelByState(state: string) {
  const labels: Record<string, string> = {
    current: "Approved",
    error: "Approval check failed",
    missing: "Needs approval",
    package_unavailable: "Package missing",
    permission_mismatch: "Permissions changed",
    stale_package_hash: "Approval outdated",
    unknown: "Unknown",
  };
  return labels[state] ?? titleCase(state.replaceAll("_", " "));
}

function titleCase(value: string) {
  return value.replace(/\b\w/g, (letter) => letter.toUpperCase());
}

export function riskVariant(risk: string) {
  if (risk === "low") return "good";
  if (risk === "medium") return "medium";
  if (risk === "high") return "red";
  return "destructive";
}

export function yesNo(value: boolean | undefined) {
  return value ? "yes" : "no";
}

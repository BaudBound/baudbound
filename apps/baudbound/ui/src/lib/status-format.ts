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

export function approvalVariant(status: ApprovalStatus) {
  if (isApprovalCurrent(status)) return "good";
  if (approvalState(status) === "missing") return "medium";
  return "destructive";
}

export function packageHashState(status: PackageHashStatus) {
  if (typeof status === "string") return status.toLowerCase();
  if ("state" in status) return status.state;
  if ("Mismatch" in status) return "mismatch";
  if ("Error" in status) return "error";
  return "unknown";
}

export function approvalState(status: ApprovalStatus) {
  if (typeof status === "string") return legacyApprovalState(status);
  if ("state" in status) return status.state;
  if ("StalePackageHash" in status) return "stale_package_hash";
  if ("Error" in status) return "error";
  return "unknown";
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

function legacyApprovalState(value: string) {
  const normalized = value.toLowerCase();
  if (normalized === "packageunavailable") return "package_unavailable";
  if (normalized === "permissionmismatch") return "permission_mismatch";
  return normalized;
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

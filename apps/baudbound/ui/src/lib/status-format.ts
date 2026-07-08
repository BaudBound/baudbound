import type { ApprovalStatus, PackageHashStatus } from "@/lib/runner-api";

export function packageHashLabel(status: PackageHashStatus) {
  if (typeof status === "string") return status.toLowerCase();
  if ("Mismatch" in status) return "mismatch";
  if ("Error" in status) return "error";
  return "unknown";
}

export function approvalLabel(status: ApprovalStatus) {
  if (typeof status === "string") return status.toLowerCase();
  if ("StalePackageHash" in status) return "stale hash";
  if ("Error" in status) return "error";
  return "unknown";
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

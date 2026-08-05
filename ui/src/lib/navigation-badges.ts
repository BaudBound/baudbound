import type { TabId } from "@/lib/app-types";
import { doctorStatus } from "@/lib/doctor-status";
import type { DashboardPayload, SystemLogSummary } from "@/lib/runner-api";

export type NavigationBadgeModel = {
  count: number;
  title: string;
  variant: "destructive" | "good" | "medium" | "muted";
};

export function navigationBadge(
  tabId: TabId,
  dashboard: DashboardPayload | null,
  systemLogs: SystemLogSummary,
): NavigationBadgeModel | null {
  if (tabId === "logs") return systemLogNavigationBadge(systemLogs);
  if (!dashboard) return null;

  if (tabId === "scripts" && dashboard.runner.problem_count > 0) {
    return {
      count: dashboard.runner.problem_count,
      title: countLabel(dashboard.runner.problem_count, "script needs review", "scripts need review"),
      variant: "medium",
    };
  }
  if (tabId === "runs" && dashboard.active_runs.length > 0) {
    return {
      count: dashboard.active_runs.length,
      title: countLabel(dashboard.active_runs.length, "active run", "active runs"),
      variant: "good",
    };
  }
  if (tabId === "diagnostics") {
    const warningCount = doctorStatus(dashboard).warningCount;
    if (warningCount > 0) {
      return {
        count: warningCount,
        title: countLabel(warningCount, "Doctor warning", "Doctor warnings"),
        variant: "medium",
      };
    }
  }
  return null;
}

export function systemLogNavigationBadge(summary: SystemLogSummary): NavigationBadgeModel | null {
  if (summary.unread_errors > 0) {
    return {
      count: summary.unread_errors,
      title: unreadSystemLogBreakdown(summary),
      variant: "destructive",
    };
  }
  if (summary.unread_warnings > 0) {
    return {
      count: summary.unread_warnings,
      title: unreadSystemLogBreakdown(summary),
      variant: "medium",
    };
  }
  if (summary.unread_info > 0) {
    return {
      count: summary.unread_info,
      title: unreadSystemLogBreakdown(summary),
      variant: "muted",
    };
  }
  if (summary.unread_successes > 0) {
    return {
      count: summary.unread_successes,
      title: unreadSystemLogBreakdown(summary),
      variant: "good",
    };
  }
  return null;
}

export function unreadSystemLogBreakdown(summary: SystemLogSummary): string {
  const parts = [
    severityCount(summary.unread_errors, "error", "errors"),
    severityCount(summary.unread_warnings, "warning", "warnings"),
    severityCount(summary.unread_info, "information", "information"),
    severityCount(summary.unread_successes, "success", "successes"),
  ].filter((part): part is string => part !== null);
  return parts.length > 0 ? `Unread system logs: ${parts.join(", ")}` : "No unread system logs";
}

function severityCount(count: number, singular: string, plural: string) {
  return count > 0 ? `${count} ${count === 1 ? singular : plural}` : null;
}

function countLabel(count: number, singular: string, plural: string) {
  return `${count} ${count === 1 ? singular : plural}`;
}

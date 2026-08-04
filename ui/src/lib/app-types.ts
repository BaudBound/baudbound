import type { ActionPayload, SystemLogDetail } from "@/lib/runner-api";

export type TabId =
  | "dashboard"
  | "browse"
  | "scripts"
  | "security"
  | "tools"
  | "runs"
  | "logs"
  | "variables"
  | "service"
  | "config"
  | "diagnostics"
  | "about";

export type Notice = {
  details?: SystemLogDetail[];
  error?: unknown;
  kind: "error" | "success";
  message: string;
  source?: string;
  title?: string;
};

export type DashboardAction = (
  actionId: string,
  action: () => Promise<ActionPayload>,
) => Promise<boolean>;

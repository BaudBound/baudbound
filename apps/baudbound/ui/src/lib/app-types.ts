import type { ActionPayload } from "@/lib/runner-api";

export type TabId =
  | "dashboard"
  | "scripts"
  | "security"
  | "triggers"
  | "tools"
  | "runs"
  | "logs"
  | "service"
  | "config"
  | "diagnostics";

export type Notice = {
  kind: "error" | "success";
  message: string;
};

export type DashboardAction = (
  actionId: string,
  action: () => Promise<ActionPayload>,
) => Promise<boolean>;

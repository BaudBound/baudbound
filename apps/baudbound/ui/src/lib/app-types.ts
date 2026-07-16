import type { ActionPayload } from "@/lib/runner-api";

export type TabId =
  | "dashboard"
  | "scripts"
  | "security"
  | "triggers"
  | "devices"
  | "runs"
  | "logs"
  | "service"
  | "config"
  | "settings"
  | "diagnostics";

export type Notice = {
  kind: "error" | "success";
  message: string;
};

export type DashboardAction = (
  actionId: string,
  action: () => Promise<ActionPayload>,
) => Promise<boolean>;

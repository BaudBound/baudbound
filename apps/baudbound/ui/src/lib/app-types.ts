import type { ActionPayload } from "@/lib/runner-api";

export type TabId =
  | "dashboard"
  | "scripts"
  | "security"
  | "tools"
  | "runs"
  | "logs"
  | "service"
  | "config"
  | "diagnostics"
  | "about";

export type Notice = {
  kind: "error" | "success";
  message: string;
};

export type DashboardAction = (
  actionId: string,
  action: () => Promise<ActionPayload>,
) => Promise<boolean>;

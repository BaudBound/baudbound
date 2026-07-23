import type { ActionPayload } from "@/lib/runner-api";

export type TabId =
  | "dashboard"
  | "browse"
  | "scripts"
  | "security"
  | "tools"
  | "runs"
  | "logs"
  | "monitor"
  | "variables"
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

import {
  ClipboardCheck,
  FileClock,
  FileCog,
  Gauge,
  ListTree,
  MonitorCog,
  ScrollText,
  ShieldCheck,
  Stethoscope,
  Wrench,
} from "lucide-react";
import type { ComponentType } from "react";

import type { TabId } from "@/lib/app-types";
import type { DashboardPayload } from "@/lib/runner-api";

export type NavigationItem = {
  icon: ComponentType<{ className?: string }>;
  id: TabId;
  label: string;
};

export const navigationGroups: Array<{ items: NavigationItem[]; label: string }> = [
  {
    label: "Operate",
    items: [
      { icon: Gauge, id: "dashboard", label: "Dashboard" },
      { icon: ScrollText, id: "scripts", label: "Scripts" },
      { icon: MonitorCog, id: "service", label: "Service" },
    ],
  },
  {
    label: "Inspect",
    items: [
      { icon: ShieldCheck, id: "security", label: "Security" },
      { icon: ListTree, id: "triggers", label: "Triggers" },
      { icon: FileClock, id: "runs", label: "Runs" },
      { icon: ClipboardCheck, id: "logs", label: "Logs" },
    ],
  },
  {
    label: "System",
    items: [
      { icon: Wrench, id: "tools", label: "Tools" },
      { icon: FileCog, id: "config", label: "Config" },
      { icon: Stethoscope, id: "diagnostics", label: "Doctor" },
    ],
  },
];

export const navigationItems = navigationGroups.flatMap((group) => group.items);

export function pageTitle(activeTab: TabId) {
  const labels: Record<TabId, string> = {
    config: "Config",
    dashboard: "Dashboard",
    diagnostics: "Doctor",
    logs: "Logs",
    runs: "Runs",
    scripts: "Scripts",
    security: "Security",
    service: "Service",
    tools: "Tools",
    triggers: "Triggers",
  };
  return labels[activeTab];
}

export function pageSubtitle(activeTab: TabId, dashboard: DashboardPayload | null) {
  if (!dashboard) return "Loading runner state...";
  if (activeTab === "scripts") {
    return `${dashboard.runner.total_script_count} installed scripts`;
  }
  if (activeTab === "security") {
    return `${dashboard.runner.problem_count} scripts need attention`;
  }
  if (activeTab === "triggers") {
    return `${dashboard.runner.trigger_count} trigger registrations`;
  }
  if (activeTab === "tools") {
    return "Utilities for inspecting and configuring the local runner";
  }
  if (activeTab === "service") {
    return `Desktop runner: ${dashboard.desktop_background.state}`;
  }
  if (activeTab === "config") {
    return dashboard.config_path;
  }
  if (activeTab === "runs") {
    return `${dashboard.recent_runs.length} recent run records`;
  }
  if (activeTab === "logs") {
    return "Recent run output and action messages";
  }
  if (activeTab === "diagnostics") {
    return "Readiness checks and troubleshooting signals";
  }
  return dashboard.storage_root;
}

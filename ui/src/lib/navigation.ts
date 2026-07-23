import {
  ClipboardCheck,
  GalleryVerticalEnd,
  FileClock,
  FileCog,
  Gauge,
  Info,
  MonitorCog,
  RadioTower,
  ScrollText,
  ShieldCheck,
  Stethoscope,
  Variable,
  Wrench,
} from "lucide-react";
import type { ComponentType } from "react";

import type { TabId } from "@/lib/app-types";
import { formatCount } from "@/lib/count-format";
import type { DashboardPayload } from "@/lib/runner-api";

export type NavigationItem = {
  icon: ComponentType<{ className?: string }>;
  id: TabId;
  label: string;
};

export const navigationGroups: Array<{
  items: NavigationItem[];
  label: string;
}> = [
  {
    label: "Operate",
    items: [
      { icon: Gauge, id: "dashboard", label: "Dashboard" },
      { icon: GalleryVerticalEnd, id: "browse", label: "Browse Scripts" },
      { icon: ScrollText, id: "scripts", label: "Scripts" },
      { icon: MonitorCog, id: "service", label: "Service" },
    ],
  },
  {
    label: "Inspect",
    items: [
      { icon: ShieldCheck, id: "security", label: "Security" },
      { icon: FileClock, id: "runs", label: "Runs" },
      { icon: ClipboardCheck, id: "logs", label: "Logs" },
      { icon: RadioTower, id: "monitor", label: "Monitor" },
      { icon: Variable, id: "variables", label: "Variables" },
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

export const utilityNavigationItems: NavigationItem[] = [
  { icon: Info, id: "about", label: "About" },
];

export const navigationItems = [
  ...navigationGroups.flatMap((group) => group.items),
  ...utilityNavigationItems,
];

export function pageTitle(activeTab: TabId) {
  const labels: Record<TabId, string> = {
    about: "About",
    browse: "Browse Scripts",
    config: "Config",
    dashboard: "Dashboard",
    diagnostics: "Doctor",
    logs: "Logs",
    monitor: "Monitor",
    runs: "Runs",
    scripts: "Scripts",
    security: "Security",
    service: "Service",
    tools: "Tools",
    variables: "Variables",
  };
  return labels[activeTab];
}

export function pageSubtitle(
  activeTab: TabId,
  dashboard: DashboardPayload | null,
) {
  if (!dashboard) return "Loading runner state...";
  if (activeTab === "about") {
    return "Application information, project links, and updates";
  }
  if (activeTab === "browse") {
    return "Discover scripts from official and user managed repositories";
  }
  if (activeTab === "scripts") {
    return formatCount(dashboard.runner.total_script_count, "installed script");
  }
  if (activeTab === "security") {
    const count = dashboard.runner.problem_count;
    return `${formatCount(count, "script")} ${count === 1 ? "needs" : "need"} attention`;
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
    return formatCount(dashboard.run_statistics.total, "retained run record");
  }
  if (activeTab === "logs") {
    return "Recent run output and action messages";
  }
  if (activeTab === "monitor") {
    return "Live input from registered triggers";
  }
  if (activeTab === "variables") {
    return "Stored values and defaults declared by installed scripts";
  }
  if (activeTab === "diagnostics") {
    return "Readiness checks and troubleshooting signals";
  }
  return "Runner status, installed scripts, and recent activity";
}

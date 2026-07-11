import {
  Activity,
  Cable,
  ClipboardCheck,
  FileClock,
  Gauge,
  HeartPulse,
  ListTree,
  MonitorCog,
  ScrollText,
  Settings,
  ShieldCheck,
  Stethoscope,
} from "lucide-react";
import {
  lazy,
  Suspense,
  type ComponentType,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { toast } from "sonner";

import { EmptyState } from "@/components/empty-state";
import { AppUpdateDialog } from "@/components/app-update-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Toaster } from "@/components/ui/sonner";
import type { DashboardAction, Notice, TabId } from "@/lib/app-types";
import {
  type ActionPayload,
  type DashboardPayload,
  getDashboardState,
} from "@/lib/runner-api";
import { DashboardView } from "@/views/dashboard-view";
import { DevicesView } from "@/views/devices-view";
import { DiagnosticsView } from "@/views/diagnostics-view";
import { LogsView } from "@/views/logs-view";
import { RunsView } from "@/views/runs-view";
import { SecurityView } from "@/views/security-view";
import { ScriptsView } from "@/views/scripts-view";
import { ServiceView } from "@/views/service-view";
import { TriggersView } from "@/views/triggers-view";

const ConfigView = lazy(() =>
  import("@/views/config-view").then((module) => ({ default: module.ConfigView })),
);

type NavigationItem = {
  icon: ComponentType<{ className?: string }>;
  id: TabId;
  label: string;
};

const navigationGroups: Array<{ items: NavigationItem[]; label: string }> = [
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
      { icon: Cable, id: "devices", label: "Devices" },
      { icon: FileClock, id: "runs", label: "Runs" },
      { icon: ClipboardCheck, id: "logs", label: "Logs" },
    ],
  },
  {
    label: "System",
    items: [
      { icon: Settings, id: "config", label: "Config" },
      { icon: Stethoscope, id: "diagnostics", label: "Doctor" },
    ],
  },
];

const navigationItems = navigationGroups.flatMap((group) => group.items);

const liveRefreshIntervalMs = 4_000;

export function App() {
  const [activeTab, setActiveTab] = useState<TabId>("dashboard");
  const [dashboard, setDashboard] = useState<DashboardPayload | null>(null);
  const [busyActions, setBusyActions] = useState<Set<string>>(new Set());
  const [lastUpdatedAt, setLastUpdatedAt] = useState<Date | null>(null);
  const refreshInFlight = useRef(false);

  const pushNotice = useCallback((notice: Notice) => {
    if (notice.kind === "success") {
      toast.success(notice.message);
    } else {
      toast.error(notice.message);
    }
  }, []);
  const reportUpdateError = useCallback(
    (message: string) => pushNotice({ kind: "error", message }),
    [pushNotice],
  );

  const refresh = useCallback(async (options?: { silent?: boolean }) => {
    if (refreshInFlight.current) return;
    refreshInFlight.current = true;
    const silent = options?.silent ?? false;
    try {
      setDashboard(await getDashboardState());
      setLastUpdatedAt(new Date());
    } catch (error) {
      if (!silent) {
        pushNotice({ kind: "error", message: String(error) });
      }
    } finally {
      refreshInFlight.current = false;
    }
  }, [pushNotice]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    const interval = window.setInterval(() => {
      void refresh({ silent: true });
    }, liveRefreshIntervalMs);

    function refreshWhenVisible() {
      if (document.visibilityState === "visible") {
        void refresh({ silent: true });
      }
    }

    window.addEventListener("focus", refreshWhenVisible);
    document.addEventListener("visibilitychange", refreshWhenVisible);
    return () => {
      window.clearInterval(interval);
      window.removeEventListener("focus", refreshWhenVisible);
      document.removeEventListener("visibilitychange", refreshWhenVisible);
    };
  }, [refresh]);

  const runAction = useCallback<DashboardAction>(
    async (actionId: string, action: () => Promise<ActionPayload>) => {
      if (busyActions.has(actionId)) return false;
      setBusyActions((current) => new Set(current).add(actionId));
      try {
        const result = await action();
        setDashboard(result.dashboard);
        setLastUpdatedAt(new Date());
        pushNotice({ kind: "success", message: result.message });
        return true;
      } catch (error) {
        pushNotice({ kind: "error", message: String(error) });
        return false;
      } finally {
        setBusyActions((current) => {
          const next = new Set(current);
          next.delete(actionId);
          return next;
        });
      }
    },
    [busyActions, pushNotice],
  );

  const activePageLabel = useMemo(() => pageTitle(activeTab), [activeTab]);
  const subtitle = useMemo(() => pageSubtitle(activeTab, dashboard), [activeTab, dashboard]);

  return (
    <div className="grid h-screen min-h-screen grid-cols-[244px_minmax(0,1fr)] bg-background text-foreground max-lg:grid-cols-1 max-lg:grid-rows-[auto_minmax(0,1fr)]">
      <aside className="flex h-screen min-h-screen flex-col border-r border-border bg-[#0a0f1a] p-3 max-lg:h-auto max-lg:min-h-0 max-lg:min-w-0 max-lg:border-b max-lg:border-r-0">
        <div className="mb-5 flex shrink-0 items-center gap-2.5 max-lg:mb-3">
          <img alt="" className="size-9 rounded-md" draggable={false} src="/logo-notext.svg" />
          <div className="min-w-0">
            <div className="truncate font-semibold">BaudBound</div>
            <div className="truncate text-xs text-muted-foreground">Desktop runner</div>
          </div>
        </div>
        <nav className="min-h-0 overflow-auto pr-1 max-lg:hidden">
          {navigationGroups.map((group) => (
            <div className="mb-4 min-w-0" key={group.label}>
              <div className="mb-1.5 px-2 text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
                {group.label}
              </div>
              <div className="grid gap-1">
                {group.items.map((tab) => {
                  const Icon = tab.icon;
                  return (
                    <Button
                      data-active={activeTab === tab.id}
                      key={tab.id}
                      onClick={() => setActiveTab(tab.id)}
                      variant="tab"
                    >
                      <Icon className="size-4" />
                      {tab.label}
                    </Button>
                  );
                })}
              </div>
            </div>
          ))}
        </nav>
        <nav className="hidden min-w-0 flex-wrap gap-1.5 max-lg:flex" aria-label="Runner sections">
          {navigationItems.map((tab) => {
            const Icon = tab.icon;
            return (
              <Button
                className="h-8 w-auto flex-none px-2.5 text-xs"
                data-active={activeTab === tab.id}
                key={tab.id}
                onClick={() => setActiveTab(tab.id)}
                variant="tab"
              >
                <Icon className="size-3.5" />
                {tab.label}
              </Button>
            );
          })}
        </nav>
      </aside>

      <main className="flex min-h-0 min-w-0 flex-col">
        <header className="flex flex-wrap items-start justify-between gap-3 border-b border-border bg-card/35 px-5 py-3 max-md:px-3">
          <div className="min-w-0 flex-1">
            <h1 className="text-xl font-semibold">{activePageLabel}</h1>
            <p className="mt-0.5 truncate text-sm text-muted-foreground">{subtitle}</p>
          </div>
          <div className="flex w-full flex-wrap justify-end gap-2 sm:w-auto">
            {dashboard ? (
              <Badge variant={dashboard.desktop_background.running ? "good" : "muted"}>
                <Activity className="mr-1 size-3" />
                {dashboard.desktop_background.running ? "Runner active" : "Runner stopped"}
              </Badge>
            ) : null}
            {lastUpdatedAt ? (
              <Badge variant="muted">
                <HeartPulse className="mr-1 size-3" />
                Updated {lastUpdatedAt.toLocaleTimeString()}
              </Badge>
            ) : null}
          </div>
        </header>

        {dashboard?.runner.problem_count ? (
          <div className="border-b border-baud-amber/25 bg-baud-amber/10 px-5 py-2 text-sm text-baud-amber max-md:px-3">
            <div className="flex items-center gap-2">
              <ShieldCheck className="size-4 shrink-0" />
              <span>
                {dashboard.runner.problem_count} script
                {dashboard.runner.problem_count === 1 ? "" : "s"} need review.
              </span>
            </div>
          </div>
        ) : null}

        <section className="min-h-0 flex-1 overflow-auto p-5 max-md:p-3">
          {!dashboard ? (
            <EmptyState>Loading runner state...</EmptyState>
          ) : activeTab === "dashboard" ? (
            <DashboardView dashboard={dashboard} />
          ) : activeTab === "scripts" ? (
            <ScriptsView
              busyActions={busyActions}
              dashboard={dashboard}
              runAction={runAction}
            />
          ) : activeTab === "security" ? (
            <SecurityView busyActions={busyActions} dashboard={dashboard} runAction={runAction} />
          ) : activeTab === "triggers" ? (
            <TriggersView
              busyActions={busyActions}
              dashboard={dashboard}
              runAction={runAction}
            />
          ) : activeTab === "devices" ? (
            <DevicesView
              busyActions={busyActions}
              dashboard={dashboard}
              runAction={runAction}
            />
          ) : activeTab === "runs" ? (
            <RunsView dashboard={dashboard} />
          ) : activeTab === "logs" ? (
            <LogsView dashboard={dashboard} />
          ) : activeTab === "service" ? (
            <ServiceView
              busyActions={busyActions}
              dashboard={dashboard}
              runAction={runAction}
            />
          ) : activeTab === "config" ? (
            <Suspense fallback={<EmptyState>Loading configuration UI...</EmptyState>}>
              <ConfigView
                busyActions={busyActions}
                dashboard={dashboard}
                runAction={runAction}
              />
            </Suspense>
          ) : (
            <DiagnosticsView dashboard={dashboard} />
          )}
        </section>
      </main>
      <AppUpdateDialog onError={reportUpdateError} />
      <Toaster closeButton position="top-center" richColors />
    </div>
  );
}

function pageTitle(activeTab: TabId) {
  const labels: Record<TabId, string> = {
    config: "Config",
    dashboard: "Dashboard",
    devices: "Devices",
    diagnostics: "Doctor",
    logs: "Logs",
    runs: "Runs",
    scripts: "Scripts",
    security: "Security",
    service: "Service",
    triggers: "Triggers",
  };
  return labels[activeTab];
}

function pageSubtitle(activeTab: TabId, dashboard: DashboardPayload | null) {
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
  if (activeTab === "devices") {
    return "Serial and device-facing runner configuration";
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
  return `${dashboard.runner.runner_name} at ${dashboard.storage_root}`;
}

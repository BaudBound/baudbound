import { listen } from "@tauri-apps/api/event";
import {
  Activity,
  HeartPulse,
  ShieldCheck,
} from "lucide-react";
import {
  lazy,
  Suspense,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { toast } from "sonner";

import { EmptyState } from "@/components/empty-state";
import { AppUpdateDialog } from "@/components/app-update-dialog";
import { DashboardLoadState } from "@/components/dashboard-load-state";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Toaster } from "@/components/ui/sonner";
import {
  applyActiveRunEvent,
  mergeActiveRunState,
  type ActiveRunState,
} from "@/lib/active-run-events";
import type { DashboardAction, Notice, TabId } from "@/lib/app-types";
import {
  navigationGroups,
  navigationItems,
  pageSubtitle,
  pageTitle,
} from "@/lib/navigation";
import {
  type ActionPayload,
  type ActiveRunEvent,
  type DashboardPayload,
  type GeneratedTriggerToken,
  getDashboardState,
} from "@/lib/runner-api";
import { createDesktopTimeFormatter, DesktopTimeProvider } from "@/lib/time-format";
import { DashboardView } from "@/views/dashboard-view";
import { DiagnosticsView } from "@/views/diagnostics-view";
import { LogsView } from "@/views/logs-view";
import { RunsView } from "@/views/runs-view";
import { SecurityView } from "@/views/security-view";
import { OneTimeTriggerTokensDialog } from "@/views/security/one-time-trigger-tokens-dialog";
import { ScriptsView } from "@/views/scripts-view";
import { ServiceView } from "@/views/service-view";
import { ToolsView } from "@/views/tools-view";

const ConfigView = lazy(() =>
  import("@/views/config-view").then((module) => ({ default: module.ConfigView })),
);
const activeRunEventChannel = "runner-active-run";
const secretVaultEventChannel = "runner-secret-vault";

export function App() {
  const [activeTab, setActiveTab] = useState<TabId>("dashboard");
  const [dashboard, setDashboard] = useState<DashboardPayload | null>(null);
  const [dashboardLoadError, setDashboardLoadError] = useState<string | null>(null);
  const [busyActions, setBusyActions] = useState<Set<string>>(new Set());
  const [lastUpdatedAt, setLastUpdatedAt] = useState<Date | null>(null);
  const [generatedTriggerTokens, setGeneratedTriggerTokens] = useState<GeneratedTriggerToken[]>([]);
  const dashboardRef = useRef<DashboardPayload | null>(null);
  const pendingActiveRunEvents = useRef<ActiveRunEvent[]>([]);
  const refreshInFlight = useRef(false);
  const refreshQueued = useRef(false);

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

  const installDashboard = useCallback((incoming: DashboardPayload) => {
    const current = dashboardRef.current;
    let activeState = mergeActiveRunState(
      current ? activeRunState(current) : activeRunState(incoming),
      activeRunState(incoming),
    );
    for (const event of pendingActiveRunEvents.current) {
      activeState = applyActiveRunEvent(activeState, event);
    }
    pendingActiveRunEvents.current = [];
    const next = withActiveRunState(incoming, activeState);
    dashboardRef.current = next;
    setDashboard(next);
  }, []);

  const refresh = useCallback(
    async (options?: { silent?: boolean }) => {
      if (refreshInFlight.current) {
        refreshQueued.current = true;
        return;
      }
      refreshInFlight.current = true;
      const silent = options?.silent ?? false;
      do {
        refreshQueued.current = false;
        if (!silent) {
          setDashboardLoadError(null);
        }
        try {
          installDashboard(await getDashboardState());
          setDashboardLoadError(null);
          setLastUpdatedAt(new Date());
        } catch (error) {
          if (!silent) {
            const message = String(error);
            setDashboardLoadError(message);
            pushNotice({ kind: "error", message });
          }
        }
      } while (refreshQueued.current);
      refreshInFlight.current = false;
    },
    [installDashboard, pushNotice],
  );

  useEffect(() => {
    let disposed = false;
    let removeListener: (() => void) | undefined;

    void listen<ActiveRunEvent>(activeRunEventChannel, (event) => {
      if (event.payload.kind === "run_recorded") {
        void refresh({ silent: true });
        return;
      }
      const current = dashboardRef.current;
      if (!current) {
        pendingActiveRunEvents.current.push(event.payload);
        return;
      }
      const currentActiveState = activeRunState(current);
      const nextActiveState = applyActiveRunEvent(currentActiveState, event.payload);
      if (nextActiveState === currentActiveState) return;
      const next = withActiveRunState(current, nextActiveState);
      dashboardRef.current = next;
      setDashboard(next);
      setLastUpdatedAt(new Date());
    })
      .then((unlisten) => {
        if (disposed) {
          unlisten();
          return;
        }
        removeListener = unlisten;
        void refresh();
      })
      .catch((error) => {
        if (!disposed) {
          pushNotice({
            kind: "error",
            message: `Could not initialize live runner events: ${String(error)}`,
          });
          void refresh();
        }
      });

    return () => {
      disposed = true;
      removeListener?.();
    };
  }, [pushNotice, refresh]);

  useEffect(() => {
    let disposed = false;
    let removeListener: (() => void) | undefined;

    void listen(secretVaultEventChannel, () => {
      void refresh({ silent: true });
    })
      .then((unlisten) => {
        if (disposed) {
          unlisten();
          return;
        }
        removeListener = unlisten;
      })
      .catch((error) => {
        if (!disposed) {
          pushNotice({
            kind: "error",
            message: `Could not initialize credential vault events: ${String(error)}`,
          });
        }
      });

    return () => {
      disposed = true;
      removeListener?.();
    };
  }, [pushNotice, refresh]);

  useEffect(() => {

    function refreshWhenVisible() {
      if (document.visibilityState === "visible") {
        void refresh({ silent: true });
      }
    }

    window.addEventListener("focus", refreshWhenVisible);
    document.addEventListener("visibilitychange", refreshWhenVisible);
    return () => {
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
        installDashboard(result.dashboard);
        const newTokens = result.generated_trigger_tokens;
        if (newTokens?.length) {
          setGeneratedTriggerTokens((current) => [
            ...current,
            ...newTokens,
          ]);
        }
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
    [busyActions, installDashboard, pushNotice],
  );

  const activePageLabel = useMemo(() => pageTitle(activeTab), [activeTab]);
  const subtitle = useMemo(() => pageSubtitle(activeTab, dashboard), [activeTab, dashboard]);
  const timeFormat = dashboard?.time_format ?? "24-hour";
  const desktopTime = useMemo(() => createDesktopTimeFormatter(timeFormat), [timeFormat]);

  return (
    <DesktopTimeProvider timeFormat={timeFormat}>
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
                Updated {desktopTime.formatTime(lastUpdatedAt)}
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

        <section className="min-h-0 min-w-0 flex-1 overflow-x-hidden overflow-y-auto p-5 max-md:p-3">
          {!dashboard ? (
            <DashboardLoadState
              error={dashboardLoadError}
              onRetry={() => void refresh()}
            />
          ) : activeTab === "dashboard" ? (
            <DashboardView dashboard={dashboard} />
          ) : activeTab === "scripts" ? (
            <ScriptsView
              busyActions={busyActions}
              dashboard={dashboard}
              runAction={runAction}
            />
          ) : activeTab === "security" ? (
            <SecurityView
              busyActions={busyActions}
              dashboard={dashboard}
              onDashboard={installDashboard}
              runAction={runAction}
            />
          ) : activeTab === "tools" ? (
            <ToolsView
              busyActions={busyActions}
              dashboard={dashboard}
              runAction={runAction}
            />
          ) : activeTab === "runs" ? (
            <RunsView
              busyActions={busyActions}
              dashboard={dashboard}
              runAction={runAction}
            />
          ) : activeTab === "logs" ? (
            <LogsView
              busyActions={busyActions}
              dashboard={dashboard}
              runAction={runAction}
            />
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
        <AppUpdateDialog
          automaticCheck={dashboard?.automatic_update_checks ?? false}
          onError={reportUpdateError}
        />
        <OneTimeTriggerTokensDialog
          onDone={() => setGeneratedTriggerTokens([])}
          tokens={generatedTriggerTokens}
        />
        <Toaster closeButton position="top-center" richColors />
      </div>
    </DesktopTimeProvider>
  );
}

function activeRunState(dashboard: DashboardPayload): ActiveRunState {
  return {
    revision: dashboard.active_runs_revision,
    runs: dashboard.active_runs,
  };
}

function withActiveRunState(
  dashboard: DashboardPayload,
  activeState: ActiveRunState,
): DashboardPayload {
  return {
    ...dashboard,
    active_runs: activeState.runs,
    active_runs_revision: activeState.revision,
  };
}

import { Activity, AlertTriangle, CheckCircle2, Clock3, PlayCircle } from "lucide-react";
import type { ReactNode } from "react";

import { Details } from "@/components/details";
import { EmptyState } from "@/components/empty-state";
import { SummaryCard } from "@/components/summary-card";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { DashboardPayload } from "@/lib/runner-api";

export function DashboardView({ dashboard }: { dashboard: DashboardPayload }) {
  const latestRuns = dashboard.recent_runs.slice(0, 5);
  const scriptsNeedingReview = dashboard.runner.scripts.filter(
    (script) => script.approval_status !== "Current" || script.package_error,
  );

  return (
    <div className="grid gap-4">
      <div className="grid grid-cols-4 gap-3 max-lg:grid-cols-2 max-sm:grid-cols-1">
        <SummaryCard label="Installed" value={dashboard.runner.total_script_count} />
        <SummaryCard label="Enabled" value={dashboard.runner.enabled_script_count} />
        <SummaryCard label="Triggers" value={dashboard.runner.trigger_count} />
        <SummaryCard label="Problems" value={dashboard.runner.problem_count} />
      </div>

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1.1fr)_minmax(360px,0.9fr)]">
        <Card>
          <CardHeader className="flex flex-row items-center justify-between gap-3">
            <CardTitle>Runner overview</CardTitle>
            <Badge variant={dashboard.desktop_background.running ? "good" : "muted"}>
              {dashboard.desktop_background.running ? "Active" : "Stopped"}
            </Badge>
          </CardHeader>
          <CardContent className="grid gap-4">
            <div className="grid gap-3 sm:grid-cols-2">
              <OverviewTile
                detail={dashboard.desktop_background.message}
                icon={<Activity className="size-4" />}
                label="Desktop loop"
                value={dashboard.desktop_background.state}
              />
              <OverviewTile
                detail={`${dashboard.runner.trigger_count} trigger registrations`}
                icon={<PlayCircle className="size-4" />}
                label="Automation"
                value={
                  dashboard.runner.enabled_script_count > 0
                    ? `${dashboard.runner.enabled_script_count} enabled`
                    : "No enabled scripts"
                }
              />
            </div>
            <Details
              rows={[
                ["Name", dashboard.runner.runner_name],
                ["Storage", dashboard.storage_root],
                [
                  "Target runtimes",
                  dashboard.runner.supported_target_runtimes.join(", "),
                ],
              ]}
            />
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex flex-row items-center justify-between gap-3">
            <CardTitle>Review queue</CardTitle>
            <Badge variant={scriptsNeedingReview.length > 0 ? "medium" : "good"}>
              {scriptsNeedingReview.length}
            </Badge>
          </CardHeader>
          <CardContent>
            {scriptsNeedingReview.length === 0 ? (
              <div className="flex gap-2 rounded-md border border-baud-green/25 bg-baud-green/10 p-3 text-sm text-baud-green">
                <CheckCircle2 className="mt-0.5 size-4 shrink-0" />
                <span>No scripts currently need approval or package review.</span>
              </div>
            ) : (
              <div className="grid gap-2">
                {scriptsNeedingReview.slice(0, 5).map((script) => (
                  <div
                    className="rounded-md border border-border bg-background px-3 py-2"
                    key={script.installed.id}
                  >
                    <div className="flex items-center justify-between gap-3">
                      <div className="min-w-0 truncate text-sm font-medium">
                        {script.installed.name}
                      </div>
                      <Badge variant="medium">{approvalLabel(script.approval_status)}</Badge>
                    </div>
                    <div className="mt-1 flex gap-2 text-xs text-baud-amber">
                      <AlertTriangle className="mt-0.5 size-3 shrink-0" />
                      <span>{script.package_error ?? "Script approval needs review."}</span>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Recent activity</CardTitle>
        </CardHeader>
        <CardContent>
          {latestRuns.length === 0 ? (
            <EmptyState>No script runs have been recorded yet.</EmptyState>
          ) : (
            <div className="grid gap-2">
              {latestRuns.map((run) => (
                <div
                  className="grid gap-3 rounded-md border border-border bg-background px-3 py-2 text-sm md:grid-cols-[180px_minmax(0,1fr)_auto]"
                  key={run.run_id}
                >
                  <div className="flex items-center gap-2 text-muted-foreground">
                    <Clock3 className="size-4" />
                    {formatUnix(run.completed_at_unix)}
                  </div>
                  <div className="min-w-0">
                    <div className="truncate font-medium">{scriptName(dashboard, run.script_id)}</div>
                    <div className="truncate font-mono text-xs text-muted-foreground">
                      {run.run_id}
                    </div>
                  </div>
                  <Badge variant={run.status === "completed" ? "good" : "destructive"}>
                    {run.status}
                  </Badge>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

function OverviewTile({
  detail,
  icon,
  label,
  value,
}: {
  detail: string;
  icon: ReactNode;
  label: string;
  value: string;
}) {
  return (
    <div className="rounded-md border border-border bg-background p-3">
      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        {icon}
        {label}
      </div>
      <div className="mt-2 text-lg font-semibold">{value}</div>
      <div className="mt-1 line-clamp-2 text-xs text-muted-foreground">{detail}</div>
    </div>
  );
}

function approvalLabel(status: DashboardPayload["runner"]["scripts"][number]["approval_status"]) {
  if (typeof status === "string") return status;
  if ("StalePackageHash" in status) return "Stale package";
  if ("Error" in status) return "Error";
  return "Unknown";
}

function scriptName(dashboard: DashboardPayload, scriptId: string) {
  return (
    dashboard.runner.scripts.find((script) => script.installed.id === scriptId)?.installed.name ??
    scriptId
  );
}

function formatUnix(value: number) {
  return new Date(value * 1000).toLocaleString();
}

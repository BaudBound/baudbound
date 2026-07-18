import { ChevronDown, ChevronUp } from "lucide-react";
import { Fragment, useMemo, useState } from "react";

import { EmptyState } from "@/components/empty-state";
import { StatusSummaryCard } from "@/components/status-summary-card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { DashboardAction } from "@/lib/app-types";
import type { DashboardPayload, StoredRunRecord } from "@/lib/runner-api";
import { nodeActionType, runStatusVariant, runSummary } from "@/lib/run-inspection";
import { useDesktopTime } from "@/lib/time-format";
import { RunDetailPanel } from "@/views/run-detail-panel";
import { ActiveRunsPanel } from "@/views/active-runs-panel";

const allFilterValue = "__all__";

export function RunsView({
  busyActions,
  dashboard,
  runAction,
}: {
  busyActions: Set<string>;
  dashboard: DashboardPayload;
  runAction: DashboardAction;
}) {
  const [expandedRunIds, setExpandedRunIds] = useState<Set<string>>(new Set());
  const [scriptFilter, setScriptFilter] = useState(allFilterValue);
  const [statusFilter, setStatusFilter] = useState(allFilterValue);
  const [searchTerm, setSearchTerm] = useState("");
  const scriptNames = useMemo(
    () =>
      new Map(
        dashboard.runner.scripts.map((script) => [
          script.installed.id,
          script.installed.name,
        ]),
      ),
    [dashboard.runner.scripts],
  );
  const statusOptions = useMemo(
    () => Array.from(new Set(dashboard.recent_runs.map((run) => run.status))).sort(),
    [dashboard.recent_runs],
  );
  const summary = useMemo(() => runSummary(dashboard.recent_runs), [dashboard.recent_runs]);
  const visibleRuns = useMemo(
    () =>
      dashboard.recent_runs.filter((run) => {
        if (scriptFilter !== allFilterValue && run.script_id !== scriptFilter) return false;
        if (statusFilter !== allFilterValue && run.status !== statusFilter) return false;
        return runMatchesSearch(run, scriptNames.get(run.script_id) ?? run.script_id, searchTerm);
      }),
    [dashboard.recent_runs, scriptFilter, scriptNames, searchTerm, statusFilter],
  );
  function toggleRunDetails(runId: string) {
    setExpandedRunIds((current) => {
      const next = new Set(current);
      if (next.has(runId)) {
        next.delete(runId);
      } else {
        next.add(runId);
      }
      return next;
    });
  }

  return (
    <div className="grid min-w-0 gap-4">
      {dashboard.recent_runs.length > 0 ? (
        <div className="status-summary-grid grid min-w-0 gap-3">
          <StatusSummaryCard label="Total" value={dashboard.recent_runs.length} />
          <StatusSummaryCard
            label="Completed"
            tone="good"
            value={summary.completed}
          />
          <StatusSummaryCard
            label="Failed"
            tone="destructive"
            value={summary.failed}
          />
          <StatusSummaryCard
            label="Cancelled"
            tone="medium"
            value={summary.cancelled}
          />
          <StatusSummaryCard
            label="With errors"
            tone="medium"
            value={summary.withErrors}
          />
        </div>
      ) : null}
      <ActiveRunsPanel
        busyActions={busyActions}
        runAction={runAction}
        runs={dashboard.active_runs}
        scriptNames={scriptNames}
      />
      {dashboard.recent_runs.length === 0 ? (
        <EmptyState>No run history has been recorded yet.</EmptyState>
      ) : (
        <Card className="min-w-0">
          <CardHeader className="grid gap-3">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <CardTitle>Recent runs</CardTitle>
              <div className="text-xs text-muted-foreground">
                Showing {visibleRuns.length} of {dashboard.recent_runs.length}
              </div>
            </div>
            <RunFilters
              onScriptFilterChange={setScriptFilter}
              onSearchTermChange={setSearchTerm}
              onStatusFilterChange={setStatusFilter}
              scriptFilter={scriptFilter}
              scripts={dashboard.runner.scripts.map((script) => ({
                id: script.installed.id,
                name: script.installed.name,
              }))}
              searchTerm={searchTerm}
              statusFilter={statusFilter}
              statusOptions={statusOptions}
            />
          </CardHeader>
          <CardContent className="min-w-0 p-0 max-[1280px]:p-3">
            {visibleRuns.length === 0 ? (
              <div className="p-4">
                <EmptyState>No runs match the current filters.</EmptyState>
              </div>
            ) : (
              <table className="responsive-table w-full table-fixed border-collapse text-sm">
                  <thead>
                    <tr className="border-b border-border text-left text-xs uppercase text-muted-foreground">
                      <th className="px-3 py-2">Completed</th>
                      <th className="px-3 py-2">Script</th>
                      <th className="px-3 py-2">Trigger</th>
                      <th className="px-3 py-2">Trigger type</th>
                      <th className="px-3 py-2">Status</th>
                      <th className="px-3 py-2">Run ID</th>
                      <th className="px-3 py-2">Recent log</th>
                    </tr>
                  </thead>
                  <tbody>
                    {visibleRuns.map((run) => {
                      const expanded = expandedRunIds.has(run.run_id);
                      const scriptName = scriptNames.get(run.script_id) ?? run.script_id;
                      return (
                        <Fragment key={run.run_id}>
                          <RunRow
                            expanded={expanded}
                            onToggleDetails={toggleRunDetails}
                            run={run}
                            scriptName={scriptName}
                          />
                          {expanded ? (
                            <tr className="border-b border-border bg-background/40">
                              <td className="p-3" colSpan={7} data-label="">
                                <RunDetailPanel run={run} scriptName={scriptName} />
                              </td>
                            </tr>
                          ) : null}
                        </Fragment>
                      );
                    })}
                  </tbody>
              </table>
            )}
          </CardContent>
        </Card>
      )}
    </div>
  );
}

function RunFilters({
  onScriptFilterChange,
  onSearchTermChange,
  onStatusFilterChange,
  scriptFilter,
  scripts,
  searchTerm,
  statusFilter,
  statusOptions,
}: {
  onScriptFilterChange: (value: string) => void;
  onSearchTermChange: (value: string) => void;
  onStatusFilterChange: (value: string) => void;
  scriptFilter: string;
  scripts: Array<{ id: string; name: string }>;
  searchTerm: string;
  statusFilter: string;
  statusOptions: string[];
}) {
  return (
    <div className="grid gap-2 lg:grid-cols-[minmax(0,1fr)_220px_180px]">
      <Input
        aria-label="Search runs"
        onChange={(event) => onSearchTermChange(event.target.value)}
        placeholder="Search run ID, script, trigger, or logs"
        value={searchTerm}
      />
      <Select onValueChange={onScriptFilterChange} value={scriptFilter}>
        <SelectTrigger aria-label="Filter by script">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value={allFilterValue}>All scripts</SelectItem>
          {scripts.map((script) => (
            <SelectItem key={script.id} value={script.id}>
              {script.name}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <Select onValueChange={onStatusFilterChange} value={statusFilter}>
        <SelectTrigger aria-label="Filter by status">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value={allFilterValue}>All statuses</SelectItem>
          {statusOptions.map((status) => (
            <SelectItem key={status} value={status}>
              {status}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}

function RunRow({
  expanded,
  onToggleDetails,
  run,
  scriptName,
}: {
  expanded: boolean;
  onToggleDetails: (runId: string) => void;
  run: StoredRunRecord;
  scriptName: string;
}) {
  const { formatUnixSeconds } = useDesktopTime();
  const lastLog = run.logs.at(-1);

  return (
    <tr
      className="border-b border-border align-top last:border-b-0 data-[expanded=true]:bg-muted/35"
      data-expanded={expanded}
    >
      <td className="px-3 py-3" data-label="Completed">
        <div className="flex items-start gap-2">
          <Button
            aria-label={`${expanded ? "Hide" : "Show"} run details`}
            className="mt-[-3px] size-7 p-0"
            onClick={() => onToggleDetails(run.run_id)}
            size="sm"
            title={expanded ? "Hide details" : "Show details"}
            variant={expanded ? "secondary" : "outline"}
          >
            {expanded ? <ChevronUp /> : <ChevronDown />}
          </Button>
          <span>{formatUnixSeconds(run.completed_at_unix)}</span>
        </div>
      </td>
      <td className="px-3 py-3" data-label="Script">
        <div className="font-medium">{scriptName}</div>
        {scriptName !== run.script_id ? (
          <div className="mt-0.5 break-all font-mono text-xs text-muted-foreground">
            {run.script_id}
          </div>
        ) : null}
      </td>
      <td className="px-3 py-3" data-label="Trigger">
        <span className="break-all">{run.trigger_node_id}</span>
      </td>
      <td
        className="px-3 py-3 font-mono text-xs text-muted-foreground"
        data-label="Trigger type"
      >
        <span className="break-all">
          {nodeActionType(run.logs, run.trigger_node_id) ?? "-"}
        </span>
      </td>
      <td className="px-3 py-3" data-label="Status">
        <Badge variant={runStatusVariant(run.status)}>{run.status}</Badge>
      </td>
      <td className="px-3 py-3" data-label="Run ID">
        <span className="break-all font-mono text-xs text-muted-foreground">
          {run.run_id}
        </span>
      </td>
      <td className="max-w-[360px] px-3 py-3" data-label="Recent log">
        {lastLog ? (
          <div className="break-words">
            <span className="text-muted-foreground">[{lastLog.level}] </span>
            {lastLog.node_id ? (
              <span className="text-muted-foreground">[{lastLog.node_id}] </span>
            ) : null}
            <span>{lastLog.message}</span>
          </div>
        ) : (
          <span className="text-muted-foreground">No logs</span>
        )}
      </td>
    </tr>
  );
}

function runMatchesSearch(run: StoredRunRecord, scriptName: string, searchTerm: string) {
  const query = searchTerm.trim().toLowerCase();
  if (!query) return true;
  const haystack = [
    run.run_id,
    run.script_id,
    scriptName,
    run.status,
    run.trigger_node_id,
    ...run.logs.flatMap((log) => [
      log.level,
      log.action_type ?? "",
      log.node_id ?? "",
      log.message,
    ]),
  ]
    .join("\n")
    .toLowerCase();
  return haystack.includes(query);
}

import { Trash2 } from "lucide-react";
import { useMemo, useState } from "react";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { EmptyState } from "@/components/empty-state";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { SortableTableHeader } from "@/components/ui/sortable-table-header";
import type { DashboardAction } from "@/lib/app-types";
import { clearRunLogs, type DashboardPayload } from "@/lib/runner-api";
import { useSortableRows } from "@/lib/table-sorting";
import { useDesktopTime } from "@/lib/time-format";

const clearLogsAction = "logs-clear";

type LogRow = {
  actionType: string | null;
  level: string;
  message: string;
  nodeId: string | null;
  runId: string;
  scriptId: string;
  scriptName: string;
  timestampUnixMs: number;
};

type LogSortColumn =
  | "level"
  | "message"
  | "node"
  | "run"
  | "script"
  | "time"
  | "type";

const logSortSelectors: Record<LogSortColumn, (row: LogRow) => number | string> = {
  level: (row) => row.level,
  message: (row) => row.message,
  node: (row) => row.nodeId ?? "runtime",
  run: (row) => row.runId,
  script: (row) => row.scriptName,
  time: (row) => row.timestampUnixMs,
  type: (row) => row.actionType ?? "",
};

export function LogsView({
  busyActions,
  dashboard,
  runAction,
}: {
  busyActions: Set<string>;
  dashboard: DashboardPayload;
  runAction: DashboardAction;
}) {
  const { formatUnixMilliseconds } = useDesktopTime();
  const [confirmClearOpen, setConfirmClearOpen] = useState(false);
  const [searchTerm, setSearchTerm] = useState("");
  const rows = useMemo(() => logRows(dashboard), [dashboard]);
  const filteredRows = useMemo(
    () => rows.filter((row) => rowMatchesSearch(row, searchTerm)),
    [rows, searchTerm],
  );
  const { sortedRows: visibleRows, sortState, toggleSort } = useSortableRows(
    filteredRows,
    logSortSelectors,
  );

  if (rows.length === 0) {
    return <EmptyState>No run logs have been recorded yet.</EmptyState>;
  }

  return (
    <div className="grid gap-4">
      <Card>
        <CardHeader className="grid gap-3">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <CardTitle>Run logs</CardTitle>
            <div className="flex flex-wrap items-center justify-end gap-3">
              <div className="text-xs text-muted-foreground">
                Showing {visibleRows.length} of {rows.length}
              </div>
              <Button
                disabled={busyActions.has(clearLogsAction)}
                onClick={() => setConfirmClearOpen(true)}
                size="sm"
                variant="outline"
              >
                <Trash2 />
                Clear logs
              </Button>
            </div>
          </div>
          <Input
            aria-label="Search logs"
            onChange={(event) => setSearchTerm(event.target.value)}
            placeholder="Search message, script, type, node, run, or level"
            value={searchTerm}
          />
        </CardHeader>
        <CardContent className="overflow-x-auto p-0 max-[1280px]:p-3">
          {visibleRows.length === 0 ? (
            <div className="p-4">
              <EmptyState>No logs match the current search.</EmptyState>
            </div>
          ) : (
            <table className="responsive-table w-full border-collapse text-sm">
              <thead>
                <tr className="border-b border-border text-left text-xs uppercase text-muted-foreground">
                  <SortableTableHeader column="time" onSort={toggleSort} sortState={sortState}>
                    Time
                  </SortableTableHeader>
                  <SortableTableHeader column="level" onSort={toggleSort} sortState={sortState}>
                    Level
                  </SortableTableHeader>
                  <SortableTableHeader column="script" onSort={toggleSort} sortState={sortState}>
                    Script
                  </SortableTableHeader>
                  <SortableTableHeader column="node" onSort={toggleSort} sortState={sortState}>
                    Node
                  </SortableTableHeader>
                  <SortableTableHeader column="type" onSort={toggleSort} sortState={sortState}>
                    Type
                  </SortableTableHeader>
                  <SortableTableHeader column="message" onSort={toggleSort} sortState={sortState}>
                    Message
                  </SortableTableHeader>
                  <SortableTableHeader column="run" onSort={toggleSort} sortState={sortState}>
                    Run
                  </SortableTableHeader>
                </tr>
              </thead>
              <tbody>
                {visibleRows.map((row, index) => (
                  <tr
                    className="border-b border-border align-top last:border-b-0"
                    key={`${row.runId}-${index}`}
                  >
                    <td className="whitespace-nowrap px-3 py-3" data-label="Time">
                      {formatUnixMilliseconds(row.timestampUnixMs)}
                    </td>
                    <td className="px-3 py-3" data-label="Level">
                      <Badge variant={logLevelVariant(row.level)}>{row.level}</Badge>
                    </td>
                    <td className="px-3 py-3" data-label="Script">
                      <div className="font-medium">{row.scriptName}</div>
                      <div className="font-mono text-xs text-muted-foreground">{row.scriptId}</div>
                    </td>
                    <td className="px-3 py-3 font-mono text-xs" data-label="Node">
                      {row.nodeId ?? "runtime"}
                    </td>
                    <td
                      className="px-3 py-3 font-mono text-xs text-muted-foreground"
                      data-label="Type"
                    >
                      {row.actionType ?? "-"}
                    </td>
                    <td className="px-3 py-3 xl:max-w-[520px]" data-label="Message">
                      {row.message}
                    </td>
                    <td
                      className="px-3 py-3 font-mono text-xs text-muted-foreground"
                      data-label="Run"
                    >
                      {row.runId}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </CardContent>
      </Card>
      <ConfirmDialog
        confirmLabel="Clear logs"
        description="Delete every stored log entry from completed runs, including runs older than those currently shown. Run records, statuses, variables, and identifiers are preserved. Running scripts can add new logs."
        destructive
        disabled={busyActions.has(clearLogsAction)}
        onConfirm={async () => {
          await runAction(clearLogsAction, clearRunLogs);
        }}
        onOpenChange={setConfirmClearOpen}
        open={confirmClearOpen}
        title="Clear stored logs?"
      />
    </div>
  );
}

function logRows(dashboard: DashboardPayload): LogRow[] {
  const scriptNames = new Map(
    dashboard.runner.scripts.map((script) => [
      script.installed.id,
      script.installed.name,
    ]),
  );

  return dashboard.recent_runs.flatMap((run) =>
    run.logs.map((log) => ({
      actionType: log.action_type ?? null,
      level: log.level,
      message: log.message,
      nodeId: log.node_id ?? null,
      runId: run.run_id,
      scriptId: run.script_id,
      scriptName: scriptNames.get(run.script_id) ?? run.script_id,
      timestampUnixMs: log.timestamp_unix_ms,
    })),
  );
}

function rowMatchesSearch(row: LogRow, searchTerm: string) {
  const query = searchTerm.trim().toLowerCase();
  if (!query) return true;
  return [
    row.actionType ?? "",
    row.level,
    row.message,
    row.nodeId ?? "",
    row.runId,
    row.scriptId,
    row.scriptName,
  ]
    .join("\n")
    .toLowerCase()
    .includes(query);
}

function logLevelVariant(level: string) {
  if (level === "error") return "destructive";
  if (level === "warn" || level === "warning") return "medium";
  if (level === "info") return "good";
  return "muted";
}

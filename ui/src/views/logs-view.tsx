import { Download, FileClock, MonitorCog, Trash2 } from "lucide-react";
import { useDeferredValue, useEffect, useState } from "react";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { EmptyState } from "@/components/empty-state";
import { PaginationControls } from "@/components/pagination-controls";
import { useSystemLog } from "@/components/system-log-provider";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { SortableTableHeader } from "@/components/ui/sortable-table-header";
import type { DashboardAction } from "@/lib/app-types";
import { formatCount } from "@/lib/count-format";
import { SEARCH_INPUT_MAX_LENGTH } from "@/lib/input-limits";
import {
  clearRunLogs,
  type DashboardPayload,
  exportLogs,
  queryRunLogs,
  type RunLogQuery,
  type StoredRunLogRecord,
  type SystemLogSummary,
} from "@/lib/runner-api";
import { unreadSystemLogBreakdown } from "@/lib/navigation-badges";
import { nextSortState, type SortState } from "@/lib/table-sorting";
import { useDesktopTime } from "@/lib/time-format";
import { visibleText } from "@/lib/visible-text";
import { SystemLogsPanel } from "@/views/system-logs-panel";

const clearLogsAction = "logs-clear";
type LogSortColumn = RunLogQuery["sort"];

export function LogsView({
  busyActions,
  dashboard,
  runAction,
}: {
  busyActions: Set<string>;
  dashboard: DashboardPayload;
  runAction: DashboardAction;
}) {
  const { openRequest, summary } = useSystemLog();
  const [section, setSection] = useState<"run" | "system">("run");

  useEffect(() => {
    if (openRequest) setSection("system");
  }, [openRequest]);

  return (
    <div className="grid gap-4">
      <div
        aria-label="Log type"
        className="grid min-w-0 grid-cols-2 overflow-hidden rounded-md border border-border bg-card"
        role="tablist"
      >
        <Button
          aria-selected={section === "run"}
          className="h-11 min-w-0 rounded-none border-0 border-r border-border"
          data-active={section === "run"}
          onClick={() => setSection("run")}
          role="tab"
          variant={section === "run" ? "secondary" : "subtle"}
        >
          <FileClock />
          Run logs
        </Button>
        <Button
          aria-selected={section === "system"}
          className="h-11 min-w-0 rounded-none border-0"
          data-active={section === "system"}
          onClick={() => setSection("system")}
          role="tab"
          variant={section === "system" ? "secondary" : "subtle"}
        >
          <MonitorCog />
          System logs
          <SystemLogUnreadBadges summary={summary} />
        </Button>
      </div>
      {section === "run" ? (
        <RunLogsPanel busyActions={busyActions} dashboard={dashboard} runAction={runAction} />
      ) : (
        <SystemLogsPanel />
      )}
    </div>
  );
}

export function SystemLogUnreadBadges({ summary }: { summary: SystemLogSummary }) {
  const badges = [
    { count: summary.unread_errors, plural: "errors", singular: "error", variant: "destructive" as const },
    { count: summary.unread_warnings, plural: "warnings", singular: "warning", variant: "medium" as const },
    { count: summary.unread_info, plural: "info", singular: "info", variant: "muted" as const },
    { count: summary.unread_successes, plural: "successes", singular: "success", variant: "good" as const },
  ].filter((badge) => badge.count > 0);
  if (badges.length === 0) return null;

  return (
    <span
      aria-label={unreadSystemLogBreakdown(summary)}
      className="ml-1 flex shrink-0 items-center gap-1"
    >
      {badges.map((badge) => {
        const label = badge.count === 1 ? badge.singular : badge.plural;
        return (
          <Badge
            aria-label={`${badge.count} unread ${label}`}
            className="min-w-5 px-1.5"
            key={badge.singular}
            title={`${badge.count} unread ${label}`}
            variant={badge.variant}
          >
            {badge.count > 99 ? "99+" : badge.count}
          </Badge>
        );
      })}
    </span>
  );
}

function RunLogsPanel({
  busyActions,
  dashboard,
  runAction,
}: {
  busyActions: Set<string>;
  dashboard: DashboardPayload;
  runAction: DashboardAction;
}) {
  const { notify } = useSystemLog();
  const { formatUnixMilliseconds } = useDesktopTime();
  const [confirmClearOpen, setConfirmClearOpen] = useState(false);
  const [search, setSearch] = useState("");
  const deferredSearch = useDeferredValue(search);
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(50);
  const [sortState, setSortState] = useState<SortState<LogSortColumn>>({
    column: "time",
    direction: "descending",
  });
  const [rows, setRows] = useState<StoredRunLogRecord[]>([]);
  const [total, setTotal] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  const query: RunLogQuery = {
    direction: sortState?.direction ?? "descending",
    limit: pageSize,
    offset: page * pageSize,
    search: deferredSearch,
    sort: sortState?.column ?? "time",
  };

  useEffect(() => {
    setPage(0);
  }, [deferredSearch, pageSize, sortState]);
  useEffect(() => {
    let cancelled = false;
    void queryRunLogs(query)
      .then((result) => {
        if (!cancelled) {
          const lastPage = Math.max(0, Math.ceil(result.total / pageSize) - 1);
          if (page > lastPage) {
            setPage(lastPage);
            return;
          }
          setRows(result.items);
          setTotal(result.total);
          setError(null);
        }
      })
      .catch((reason) => {
        if (!cancelled) setError(String(reason));
      });
    return () => {
      cancelled = true;
    };
  }, [
    dashboard.run_statistics.total,
    dashboard.active_runs_revision,
    dashboard.recent_runs,
    page,
    pageSize,
    query.direction,
    query.limit,
    query.offset,
    query.search,
    query.sort,
  ]);

  function toggleSort(column: LogSortColumn) {
    setSortState((current) => nextSortState(current, column));
  }

  async function exportMatching(format: "csv" | "json") {
    setExporting(true);
    try {
      const result = await exportLogs(format, { ...query, offset: 0 });
      if (!result.cancelled)
        notify.success(`Exported ${formatCount(result.exported_count, "log")} to ${result.file_name}.`, {
          source: "Run logs",
          title: "Run logs exported",
        });
    } catch (reason) {
      notify.error("The run logs could not be exported.", {
        error: reason,
        source: "Run logs",
        title: "Run log export failed",
      });
    } finally {
      setExporting(false);
    }
  }

  return (
    <>
      <Card>
        <CardHeader className="grid gap-3">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <CardTitle>Run logs</CardTitle>
            <div className="flex flex-wrap gap-2">
              <Button
                disabled={exporting || total === 0}
                onClick={() => void exportMatching("json")}
                size="sm"
                variant="outline"
              >
                <Download />
                Export JSON
              </Button>
              <Button
                disabled={exporting || total === 0}
                onClick={() => void exportMatching("csv")}
                size="sm"
                variant="outline"
              >
                <Download />
                Export CSV
              </Button>
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
            maxLength={SEARCH_INPUT_MAX_LENGTH}
            onChange={(event) => setSearch(event.target.value)}
            placeholder="Search message, script, type, node, run, or level"
            value={search}
          />
        </CardHeader>
        <CardContent className="overflow-x-auto p-0 max-[1280px]:p-3">
          {error ? (
            <div className="p-4">
              <EmptyState>Could not load logs: {error}</EmptyState>
            </div>
          ) : rows.length === 0 ? (
            <div className="p-4">
              <EmptyState>No logs match the current search.</EmptyState>
            </div>
          ) : (
            <table className="responsive-table min-w-[1120px] w-full border-collapse text-sm max-[1280px]:min-w-0">
              <thead>
                <tr className="border-b border-border text-left text-xs uppercase text-muted-foreground">
                  {(
                    [
                      ["time", "Time"],
                      ["level", "Level"],
                      ["script", "Script"],
                      ["node", "Node"],
                      ["type", "Type"],
                      ["message", "Message"],
                      ["run", "Run"],
                    ] as const
                  ).map(([column, label]) => (
                    <SortableTableHeader column={column} key={column} onSort={toggleSort} sortState={sortState}>
                      {label}
                    </SortableTableHeader>
                  ))}
                </tr>
              </thead>
              <tbody>
                {rows.map((row) => (
                  <tr className="border-b border-border align-top last:border-0" key={`${row.run_id}-${row.log_index}`}>
                    <td className="whitespace-nowrap px-3 py-3" data-label="Time">
                      {formatUnixMilliseconds(row.timestamp_unix_ms)}
                    </td>
                    <td className="px-3 py-3" data-label="Level">
                      <Badge variant={logLevelVariant(row.level)}>{row.level}</Badge>
                    </td>
                    <td className="px-3 py-3" data-label="Script">
                      <div className="font-medium">{row.script_name}</div>
                      <div className="break-words font-mono text-xs text-muted-foreground">{row.script_id}</div>
                    </td>
                    <td className="px-3 py-3 font-mono text-xs" data-label="Node">
                      {row.node_id ?? "runtime"}
                    </td>
                    <td className="px-3 py-3 font-mono text-xs text-muted-foreground" data-label="Type">
                      {row.action_type ?? "-"}
                    </td>
                    <td className="px-3 py-3 xl:max-w-[520px]" data-label="Message">
                      <span className="select-text break-words font-mono text-xs">{visibleText(row.message)}</span>
                    </td>
                    <td className="break-words px-3 py-3 font-mono text-xs text-muted-foreground" data-label="Run">
                      {row.run_id}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </CardContent>
        <PaginationControls
          onPageChange={setPage}
          onPageSizeChange={setPageSize}
          page={page}
          pageSize={pageSize}
          total={total}
        />
      </Card>
      <ConfirmDialog
        confirmLabel="Clear logs"
        description="Delete every stored log entry from completed runs. Run records, statuses, variables, and identifiers are preserved."
        destructive
        disabled={busyActions.has(clearLogsAction)}
        onConfirm={async () => {
          await runAction(clearLogsAction, clearRunLogs);
        }}
        onOpenChange={setConfirmClearOpen}
        open={confirmClearOpen}
        title="Clear stored logs?"
      />
    </>
  );
}

function logLevelVariant(level: string) {
  if (level === "error") return "destructive";
  if (level === "warn" || level === "warning") return "medium";
  if (level === "info") return "good";
  return "muted";
}

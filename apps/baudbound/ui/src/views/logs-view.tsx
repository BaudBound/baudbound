import { Download, Trash2 } from "lucide-react";
import { useDeferredValue, useEffect, useState } from "react";
import { toast } from "sonner";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { EmptyState } from "@/components/empty-state";
import { PaginationControls } from "@/components/pagination-controls";
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
  exportLogs,
  queryRunLogs,
  type DashboardPayload,
  type RunLogQuery,
  type StoredRunLogRecord,
} from "@/lib/runner-api";
import { nextSortState, type SortState } from "@/lib/table-sorting";
import { useDesktopTime } from "@/lib/time-format";
import { visibleText } from "@/lib/visible-text";

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
        toast.success(
          `Exported ${formatCount(result.exported_count, "log")} to ${result.file_name}.`,
        );
    } catch (reason) {
      toast.error(`Could not export logs: ${String(reason)}`);
    } finally {
      setExporting(false);
    }
  }

  return (
    <div className="grid gap-4">
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
            <table className="responsive-table w-full border-collapse text-sm">
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
                    <SortableTableHeader
                      column={column}
                      key={column}
                      onSort={toggleSort}
                      sortState={sortState}
                    >
                      {label}
                    </SortableTableHeader>
                  ))}
                </tr>
              </thead>
              <tbody>
                {rows.map((row) => (
                  <tr
                    className="border-b border-border align-top last:border-0"
                    key={`${row.run_id}-${row.log_index}`}
                  >
                    <td
                      className="whitespace-nowrap px-3 py-3"
                      data-label="Time"
                    >
                      {formatUnixMilliseconds(row.timestamp_unix_ms)}
                    </td>
                    <td className="px-3 py-3" data-label="Level">
                      <Badge variant={logLevelVariant(row.level)}>
                        {row.level}
                      </Badge>
                    </td>
                    <td className="px-3 py-3" data-label="Script">
                      <div className="font-medium">{row.script_name}</div>
                      <div className="break-all font-mono text-xs text-muted-foreground">
                        {row.script_id}
                      </div>
                    </td>
                    <td
                      className="px-3 py-3 font-mono text-xs"
                      data-label="Node"
                    >
                      {row.node_id ?? "runtime"}
                    </td>
                    <td
                      className="px-3 py-3 font-mono text-xs text-muted-foreground"
                      data-label="Type"
                    >
                      {row.action_type ?? "-"}
                    </td>
                    <td
                      className="px-3 py-3 xl:max-w-[520px]"
                      data-label="Message"
                    >
                      <span className="break-words font-mono text-xs">
                        {visibleText(row.message)}
                      </span>
                    </td>
                    <td
                      className="break-all px-3 py-3 font-mono text-xs text-muted-foreground"
                      data-label="Run"
                    >
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
    </div>
  );
}

function logLevelVariant(level: string) {
  if (level === "error") return "destructive";
  if (level === "warn" || level === "warning") return "medium";
  if (level === "info") return "good";
  return "muted";
}

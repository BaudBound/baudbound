import { Copy, Download, Info, Trash2 } from "lucide-react";
import { useDeferredValue, useEffect, useState } from "react";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { EmptyState } from "@/components/empty-state";
import { PaginationControls } from "@/components/pagination-controls";
import { useSystemLog } from "@/components/system-log-provider";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { SortableTableHeader } from "@/components/ui/sortable-table-header";
import { formatCount } from "@/lib/count-format";
import { SEARCH_INPUT_MAX_LENGTH } from "@/lib/input-limits";
import {
  exportSystemLogs,
  getSystemLog,
  querySystemLogs,
  type StoredSystemLog,
  type SystemLogQuery,
  type SystemLogSeverity,
} from "@/lib/runner-api";
import { formatSystemLogDetails } from "@/lib/system-log-model";
import { nextSortState, type SortState } from "@/lib/table-sorting";
import { useDesktopTime } from "@/lib/time-format";
import { visibleText } from "@/lib/visible-text";

type SystemLogSortColumn = SystemLogQuery["sort"];
type SeverityFilter = SystemLogSeverity | "all";

export function SystemLogsPanel() {
  const { clear, clearOpenRequest, markAllRead, notify, openRequest, revision, setViewing, summary } = useSystemLog();
  const { formatUnixMilliseconds } = useDesktopTime();
  const [confirmClearOpen, setConfirmClearOpen] = useState(false);
  const [selectedLog, setSelectedLog] = useState<StoredSystemLog | null>(null);
  const [search, setSearch] = useState("");
  const deferredSearch = useDeferredValue(search);
  const [severity, setSeverity] = useState<SeverityFilter>("all");
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(50);
  const [sortState, setSortState] = useState<SortState<SystemLogSortColumn>>({
    column: "time",
    direction: "descending",
  });
  const [rows, setRows] = useState<StoredSystemLog[]>([]);
  const [total, setTotal] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  const query: SystemLogQuery = {
    direction: sortState?.direction ?? "descending",
    limit: pageSize,
    offset: page * pageSize,
    search: deferredSearch,
    severity: severity === "all" ? null : severity,
    sort: sortState?.column ?? "time",
  };

  useEffect(() => {
    setViewing(true);
    void markAllRead();
    return () => setViewing(false);
  }, [markAllRead, setViewing]);

  useEffect(() => {
    setPage(0);
  }, [deferredSearch, pageSize, severity, sortState]);

  useEffect(() => {
    let cancelled = false;
    void querySystemLogs(query)
      .then((result) => {
        if (cancelled) return;
        const lastPage = Math.max(0, Math.ceil(result.total / pageSize) - 1);
        if (page > lastPage) {
          setPage(lastPage);
          return;
        }
        setRows(result.items);
        setTotal(result.total);
        setError(null);
      })
      .catch((reason) => {
        if (!cancelled) setError(String(reason));
      });
    return () => {
      cancelled = true;
    };
  }, [page, pageSize, query.direction, query.limit, query.offset, query.search, query.severity, query.sort, revision]);

  useEffect(() => {
    if (!openRequest) return;
    let cancelled = false;
    void getSystemLog(openRequest.id)
      .then((log) => {
        if (cancelled) return;
        if (log) {
          setSelectedLog(log);
        } else {
          notify.warning("The selected system log is no longer retained.", {
            source: "System logs",
            title: "Log unavailable",
          });
        }
      })
      .catch((reason) => {
        if (!cancelled) {
          notify.error("The selected system log could not be loaded.", {
            error: reason,
            source: "System logs",
            title: "Could not load log details",
          });
        }
      })
      .finally(() => clearOpenRequest(openRequest));
    return () => {
      cancelled = true;
    };
  }, [clearOpenRequest, notify, openRequest]);

  function toggleSort(column: SystemLogSortColumn) {
    setSortState((current) => nextSortState(current, column));
  }

  async function exportMatching() {
    setExporting(true);
    try {
      const result = await exportSystemLogs({ ...query, offset: 0 });
      if (!result.cancelled) {
        notify.success(`Exported ${formatCount(result.exported_count, "system log")} to ${result.file_name}.`, {
          source: "System logs",
          title: "System logs exported",
        });
      }
    } catch (reason) {
      notify.error("The system logs could not be exported.", {
        error: reason,
        source: "System logs",
        title: "System log export failed",
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
            <CardTitle>System logs</CardTitle>
            <div className="flex flex-wrap gap-2">
              <Button
                disabled={exporting || total === 0}
                onClick={() => void exportMatching()}
                size="sm"
                variant="outline"
              >
                <Download />
                Export JSON
              </Button>
              <Button
                disabled={summary.total === 0}
                onClick={() => setConfirmClearOpen(true)}
                size="sm"
                variant="outline"
              >
                <Trash2 />
                Clear logs
              </Button>
            </div>
          </div>
          <div className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_180px]">
            <Input
              aria-label="Search system logs"
              maxLength={SEARCH_INPUT_MAX_LENGTH}
              onChange={(event) => setSearch(event.target.value)}
              placeholder="Search source, title, message, or details"
              value={search}
            />
            <Select onValueChange={(value) => setSeverity(value as SeverityFilter)} value={severity}>
              <SelectTrigger aria-label="Filter system logs by severity">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All severities</SelectItem>
                <SelectItem value="error">Errors</SelectItem>
                <SelectItem value="warning">Warnings</SelectItem>
                <SelectItem value="info">Information</SelectItem>
                <SelectItem value="success">Success</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </CardHeader>
        <CardContent className="overflow-x-hidden p-0 max-[1100px]:p-3">
          {error ? (
            <div className="p-4">
              <EmptyState>Could not load system logs: {error}</EmptyState>
            </div>
          ) : rows.length === 0 ? (
            <div className="p-4">
              <EmptyState>No system logs match the current filters.</EmptyState>
            </div>
          ) : (
            <table className="responsive-table w-full border-collapse text-sm">
              <thead>
                <tr className="border-b border-border text-left text-xs uppercase text-muted-foreground">
                  <SortableTableHeader column="time" onSort={toggleSort} sortState={sortState}>
                    Time
                  </SortableTableHeader>
                  <SortableTableHeader column="severity" onSort={toggleSort} sortState={sortState}>
                    Severity
                  </SortableTableHeader>
                  <SortableTableHeader column="source" onSort={toggleSort} sortState={sortState}>
                    Source
                  </SortableTableHeader>
                  <SortableTableHeader column="title" onSort={toggleSort} sortState={sortState}>
                    Log
                  </SortableTableHeader>
                  <th className="w-16 px-3 py-2 text-right font-medium">Details</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((row) => (
                  <tr className="border-b border-border align-top last:border-0" key={row.id}>
                    <td className="whitespace-nowrap px-3 py-3" data-label="Time">
                      <div className="flex items-center gap-2">
                        {row.unread ? (
                          <span
                            aria-label="Unread"
                            className="size-1.5 shrink-0 rounded-full bg-baud-red"
                            title="Unread"
                          />
                        ) : null}
                        {formatUnixMilliseconds(row.timestamp_unix_ms)}
                      </div>
                    </td>
                    <td className="px-3 py-3" data-label="Severity">
                      <Badge variant={severityVariant(row.severity)}>{severityLabel(row.severity)}</Badge>
                    </td>
                    <td className="px-3 py-3" data-label="Source">
                      <span className="break-words font-medium">{row.source}</span>
                    </td>
                    <td className="px-3 py-3 xl:max-w-[620px]" data-label="Log">
                      <div className="select-text break-words font-medium">{row.title}</div>
                      <div className="mt-1 select-text whitespace-pre-wrap break-words text-xs text-muted-foreground">
                        {visibleText(row.message)}
                      </div>
                    </td>
                    <td className="px-3 py-3 text-right" data-label="Details">
                      <Button
                        aria-label={`View details for ${row.title}`}
                        className="size-8 p-0"
                        onClick={() => setSelectedLog(row)}
                        title="View details"
                        variant="outline"
                      >
                        <Info />
                      </Button>
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
        confirmLabel="Clear system logs"
        description="Delete every retained runner system log. Run history and script output are not affected."
        destructive
        disabled={summary.total === 0}
        onConfirm={async () => {
          await clear();
          setSelectedLog(null);
        }}
        onOpenChange={setConfirmClearOpen}
        open={confirmClearOpen}
        title="Clear system logs?"
      />
      <SystemLogDetailsDialog log={selectedLog} onOpenChange={(open) => !open && setSelectedLog(null)} />
    </>
  );
}

function SystemLogDetailsDialog({
  log,
  onOpenChange,
}: {
  log: StoredSystemLog | null;
  onOpenChange: (open: boolean) => void;
}) {
  const { notify } = useSystemLog();
  const { formatUnixMilliseconds } = useDesktopTime();

  async function copyDetails() {
    if (!log) return;
    try {
      await navigator.clipboard.writeText(formatSystemLogDetails(log));
      notify.success("The complete system log was copied to the clipboard.", {
        source: "System logs",
        title: "Log details copied",
      });
    } catch (error) {
      notify.error("The system log details could not be copied.", {
        error,
        source: "System logs",
        title: "Could not copy log details",
      });
    }
  }

  return (
    <Dialog onOpenChange={onOpenChange} open={log !== null}>
      <DialogContent className="max-h-[min(760px,calc(100vh-2rem))] w-[min(calc(100vw-2rem),760px)] grid-rows-[auto_minmax(0,1fr)_auto] overflow-hidden">
        <DialogHeader>
          <DialogTitle>{log?.title ?? "System log details"}</DialogTitle>
          <DialogDescription>Complete information retained for this runner event.</DialogDescription>
        </DialogHeader>
        {log ? (
          <div className="min-h-0 select-text overflow-y-auto pr-2 text-sm">
            <dl className="grid gap-4">
              <LogDetail label="Time" value={formatUnixMilliseconds(log.timestamp_unix_ms)} />
              <LogDetail label="Severity" value={severityLabel(log.severity)} />
              <LogDetail label="Source" value={log.source} />
              <LogDetail label="Message" value={visibleText(log.message)} preserveWhitespace />
              {log.details.map((detail, index) => (
                <LogDetail
                  key={`${detail.label}-${index}`}
                  label={detail.label}
                  preserveWhitespace
                  value={visibleText(detail.value)}
                />
              ))}
              <LogDetail label="Log ID" value={log.id} />
            </dl>
          </div>
        ) : null}
        <DialogFooter>
          <Button onClick={() => onOpenChange(false)} variant="outline">
            Close
          </Button>
          <Button onClick={() => void copyDetails()}>
            <Copy />
            Copy all details
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function LogDetail({
  label,
  preserveWhitespace = false,
  value,
}: {
  label: string;
  preserveWhitespace?: boolean;
  value: string;
}) {
  return (
    <div className="grid gap-1 border-b border-border pb-3 last:border-0">
      <dt className="text-xs font-medium uppercase text-muted-foreground">{label}</dt>
      <dd
        className={preserveWhitespace ? "whitespace-pre-wrap break-words font-mono text-xs leading-5" : "break-words"}
      >
        {value || "-"}
      </dd>
    </div>
  );
}

function severityVariant(severity: SystemLogSeverity) {
  if (severity === "error") return "destructive" as const;
  if (severity === "warning") return "medium" as const;
  if (severity === "success") return "good" as const;
  return "muted" as const;
}

function severityLabel(severity: SystemLogSeverity) {
  return severity === "info" ? "Information" : `${severity[0].toUpperCase()}${severity.slice(1)}`;
}

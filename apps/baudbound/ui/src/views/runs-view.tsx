import { Download, Eye, Trash2, X } from "lucide-react";
import {
  useDeferredValue,
  useEffect,
  useMemo,
  useState,
} from "react";
import { toast } from "sonner";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { DetailDialog } from "@/components/detail-dialog";
import { EmptyState } from "@/components/empty-state";
import { PaginationControls } from "@/components/pagination-controls";
import { StatusSummaryCard } from "@/components/status-summary-card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { SortableTableHeader } from "@/components/ui/sortable-table-header";
import type { DashboardAction } from "@/lib/app-types";
import { formatCount } from "@/lib/count-format";
import {
  clearRunHistory,
  exportRuns,
  queryRunHistory,
  type DashboardPayload,
  type RunHistoryQuery,
  type StoredRunRecord,
} from "@/lib/runner-api";
import { nodeActionType, runStatusVariant } from "@/lib/run-inspection";
import { nextSortState, type SortState } from "@/lib/table-sorting";
import { useDesktopTime } from "@/lib/time-format";
import { visibleText } from "@/lib/visible-text";
import { ActiveRunsPanel } from "@/views/active-runs-panel";
import { RunDetailPanel } from "@/views/run-detail-panel";

const all = "__all__";
const clearHistoryAction = "runs-clear-history";
type RunSortColumn = RunHistoryQuery["sort"];

export function RunsView({
  busyActions,
  dashboard,
  runAction,
}: {
  busyActions: Set<string>;
  dashboard: DashboardPayload;
  runAction: DashboardAction;
}) {
  const [detailRun, setDetailRun] = useState<StoredRunRecord | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [confirmClearOpen, setConfirmClearOpen] = useState(false);
  const [scriptFilter, setScriptFilter] = useState(all);
  const [statusFilter, setStatusFilter] = useState(all);
  const [search, setSearch] = useState("");
  const deferredSearch = useDeferredValue(search);
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(50);
  const [sortState, setSortState] = useState<SortState<RunSortColumn>>({
    column: "completed",
    direction: "descending",
  });
  const [runs, setRuns] = useState<StoredRunRecord[]>([]);
  const [total, setTotal] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
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
  const query: RunHistoryQuery = {
    direction: sortState?.direction ?? "descending",
    limit: pageSize,
    offset: page * pageSize,
    script_id: scriptFilter === all ? null : scriptFilter,
    search: deferredSearch,
    sort: sortState?.column ?? "completed",
    status: statusFilter === all ? null : statusFilter,
  };

  useEffect(() => {
    setPage(0);
  }, [deferredSearch, pageSize, scriptFilter, sortState, statusFilter]);
  useEffect(() => {
    let cancelled = false;
    void queryRunHistory(query)
      .then((result) => {
        if (!cancelled) {
          const lastPage = Math.max(0, Math.ceil(result.total / pageSize) - 1);
          if (page > lastPage) {
            setPage(lastPage);
            return;
          }
          setRuns(result.items);
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
    page,
    pageSize,
    query.direction,
    query.limit,
    query.offset,
    query.script_id,
    query.search,
    query.sort,
    query.status,
  ]);

  const pageIds = runs.map((run) => run.run_id);
  const selectedOnPage = pageIds.filter((id) => selected.has(id)).length;
  const canSelectAllMatching =
    pageIds.length > 0 &&
    selectedOnPage === pageIds.length &&
    total > selected.size;
  const pageChecked =
    pageIds.length === 0 || selectedOnPage === 0
      ? false
      : selectedOnPage === pageIds.length
        ? true
        : "indeterminate";

  function toggleSort(column: RunSortColumn) {
    setSortState((current) => nextSortState(current, column));
  }
  function toggleSelected(runId: string) {
    setSelected((current) => toggleSet(current, runId));
  }
  function selectPage(checked: boolean) {
    setSelected((current) => {
      const next = new Set(current);
      for (const id of pageIds) checked ? next.add(id) : next.delete(id);
      return next;
    });
  }

  async function selectAllMatching() {
    const ids = new Set<string>();
    let offset = 0;
    while (offset < total) {
      const result = await queryRunHistory({ ...query, limit: 200, offset });
      result.items.forEach((run) => ids.add(run.run_id));
      offset += result.items.length;
      if (result.items.length === 0) break;
    }
    setSelected(ids);
  }

  async function exportSelected() {
    setExporting(true);
    try {
      const result = await exportRuns([...selected]);
      if (!result.cancelled)
        toast.success(
          `Exported ${formatCount(result.exported_count, "run")} to ${result.file_name}.`,
        );
    } catch (reason) {
      toast.error(`Could not export runs: ${String(reason)}`);
    } finally {
      setExporting(false);
    }
  }

  return (
    <div className="grid min-w-0 gap-4">
      {dashboard.run_statistics.total > 0 ? (
        <section className="grid min-w-0 gap-2">
          <h2 className="text-sm font-medium">Retained run history</h2>
          <div className="status-summary-grid grid min-w-0 gap-3">
            <StatusSummaryCard
              label="Total"
              value={dashboard.run_statistics.total}
            />
            <StatusSummaryCard
              label="Completed"
              tone="good"
              value={dashboard.run_statistics.completed}
            />
            <StatusSummaryCard
              label="Failed"
              tone="destructive"
              value={dashboard.run_statistics.failed}
            />
            <StatusSummaryCard
              label="Cancelled"
              tone="medium"
              value={dashboard.run_statistics.cancelled}
            />
            <StatusSummaryCard
              label="With errors"
              tone="medium"
              value={dashboard.run_statistics.with_errors}
            />
          </div>
        </section>
      ) : null}
      <ActiveRunsPanel
        busyActions={busyActions}
        runAction={runAction}
        runs={dashboard.active_runs}
        scriptNames={scriptNames}
      />
      <Card className="min-w-0">
        <CardHeader className="grid gap-3">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <CardTitle>Recent runs</CardTitle>
            <div className="flex flex-wrap items-center justify-end gap-2">
              <Button
                aria-hidden={!canSelectAllMatching}
                className={canSelectAllMatching ? undefined : "invisible"}
                disabled={!canSelectAllMatching}
                onClick={() => void selectAllMatching()}
                size="sm"
                variant="subtle"
              >
                Select all {total} matching
              </Button>
              <Button
                disabled={selected.size === 0}
                onClick={() => setSelected(new Set())}
                size="sm"
                variant="subtle"
              >
                <X />
                Clear selection
              </Button>
              <Button
                className="whitespace-nowrap"
                disabled={exporting || selected.size === 0}
                onClick={() => void exportSelected()}
                size="sm"
                variant="outline"
              >
                <Download />
                Export selected ({selected.size})
              </Button>
              <Button
                disabled={busyActions.has(clearHistoryAction)}
                onClick={() => setConfirmClearOpen(true)}
                size="sm"
                variant="outline"
              >
                <Trash2 />
                Clear runs
              </Button>
            </div>
          </div>
          <div className="grid gap-2 lg:grid-cols-[minmax(0,1fr)_220px_180px]">
            <Input
              aria-label="Search runs"
              onChange={(event) => setSearch(event.target.value)}
              placeholder="Search run ID, script, trigger, or logs"
              value={search}
            />
            <Select onValueChange={setScriptFilter} value={scriptFilter}>
              <SelectTrigger aria-label="Filter by script">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={all}>All scripts</SelectItem>
                {dashboard.runner.scripts.map((script) => (
                  <SelectItem
                    key={script.installed.id}
                    value={script.installed.id}
                  >
                    {script.installed.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Select onValueChange={setStatusFilter} value={statusFilter}>
              <SelectTrigger aria-label="Filter by status">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={all}>All statuses</SelectItem>
                {["completed", "failed", "cancelled"].map((status) => (
                  <SelectItem key={status} value={status}>
                    {status}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </CardHeader>
        <CardContent className="min-w-0 p-0 max-[1280px]:p-3">
          {error ? (
            <div className="p-4">
              <EmptyState>Could not load runs: {error}</EmptyState>
            </div>
          ) : runs.length === 0 ? (
            <div className="p-4">
              <EmptyState>No runs match the current filters.</EmptyState>
            </div>
          ) : (
            <table className="responsive-table w-full table-fixed border-collapse text-sm">
              <thead>
                <tr className="border-b border-border text-left text-xs uppercase text-muted-foreground">
                  <th className="w-11 px-3 py-2">
                    <Checkbox
                      aria-label="Select current page"
                      checked={pageChecked}
                      onCheckedChange={(value) => selectPage(value === true)}
                    />
                  </th>
                  {(
                    [
                      ["completed", "Completed"],
                      ["script", "Script"],
                      ["trigger", "Trigger"],
                      ["trigger_type", "Trigger type"],
                      ["status", "Status"],
                      ["run_id", "Run ID"],
                      ["recent_log", "Recent log"],
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
                  <th className="w-14 px-3 py-2 text-right">
                    <span className="sr-only">Details</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                {runs.map((run) => (
                  <RunRow
                    key={run.run_id}
                    onToggleSelected={() => toggleSelected(run.run_id)}
                    onView={() => setDetailRun(run)}
                    run={run}
                    scriptName={scriptNames.get(run.script_id) ?? run.script_id}
                    selected={selected.has(run.run_id)}
                  />
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
          selectedCount={selected.size}
          total={total}
        />
      </Card>
      <ConfirmDialog
        confirmLabel="Clear runs"
        description="Delete all stored completed run records. Running scripts are not stopped and can create new records when they finish."
        destructive
        disabled={busyActions.has(clearHistoryAction)}
        onConfirm={async () => {
          const cleared = await runAction(clearHistoryAction, clearRunHistory);
          if (cleared) {
            setDetailRun(null);
            setSelected(new Set());
          }
        }}
        onOpenChange={setConfirmClearOpen}
        open={confirmClearOpen}
        title="Clear run history?"
      />
      <DetailDialog
        description={
          detailRun
            ? `${scriptNames.get(detailRun.script_id) ?? detailRun.script_id} | ${detailRun.run_id}`
            : "Run information"
        }
        onOpenChange={(open) => {
          if (!open) setDetailRun(null);
        }}
        open={detailRun !== null}
        title="Run details"
      >
        {detailRun ? (
          <RunDetailPanel
            run={detailRun}
            scriptName={scriptNames.get(detailRun.script_id) ?? detailRun.script_id}
          />
        ) : null}
      </DetailDialog>
    </div>
  );
}

function RunRow({
  onToggleSelected,
  onView,
  run,
  scriptName,
  selected,
}: {
  onToggleSelected: () => void;
  onView: () => void;
  run: StoredRunRecord;
  scriptName: string;
  selected: boolean;
}) {
  const { formatUnixSeconds } = useDesktopTime();
  const lastLog = run.logs.at(-1);
  return (
    <tr className="border-b border-border align-top last:border-0">
      <td className="px-3 py-3" data-label="Select">
        <Checkbox
          aria-label={`Select run ${run.run_id}`}
          checked={selected}
          onCheckedChange={onToggleSelected}
        />
      </td>
      <td className="px-3 py-3" data-label="Completed">
        {formatUnixSeconds(run.completed_at_unix)}
      </td>
      <td className="px-3 py-3" data-label="Script">
        <div className="font-medium">{scriptName}</div>
        <div className="break-all font-mono text-xs text-muted-foreground">
          {run.script_id}
        </div>
      </td>
      <td className="break-all px-3 py-3" data-label="Trigger">
        {run.trigger_node_id}
      </td>
      <td
        className="break-all px-3 py-3 font-mono text-xs text-muted-foreground"
        data-label="Trigger type"
      >
        {nodeActionType(run.logs, run.trigger_node_id) ?? "-"}
      </td>
      <td className="px-3 py-3" data-label="Status">
        <Badge variant={runStatusVariant(run.status)}>{run.status}</Badge>
      </td>
      <td
        className="break-all px-3 py-3 font-mono text-xs text-muted-foreground"
        data-label="Run ID"
      >
        {run.run_id}
      </td>
      <td className="max-w-[360px] px-3 py-3" data-label="Recent log">
        {lastLog ? (
          <div className="break-words">
            <span className="text-muted-foreground">[{lastLog.level}] </span>
            <span className="font-mono text-xs">
              {visibleText(lastLog.message)}
            </span>
          </div>
        ) : (
          <span className="text-muted-foreground">No logs</span>
        )}
      </td>
      <td className="px-3 py-3 text-right" data-label="Details">
        <Button
          aria-label={`View details for run ${run.run_id}`}
          className="size-8 p-0"
          onClick={onView}
          size="sm"
          title="View details"
          variant="outline"
        >
          <Eye />
        </Button>
      </td>
    </tr>
  );
}

function toggleSet(current: Set<string>, value: string) {
  const next = new Set(current);
  next.has(value) ? next.delete(value) : next.add(value);
  return next;
}

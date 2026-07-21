import { Activity, ListTree } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { SortableTableHeader } from "@/components/ui/sortable-table-header";
import type {
  DashboardPayload,
  TriggerDispatchActivity,
  TriggerRegistrationStatus,
} from "@/lib/runner-api";
import { useDesktopTime } from "@/lib/time-format";
import { useSortableRows } from "@/lib/table-sorting";
import {
  SerialReaderStatusPanel,
  type SerialTriggerRegistration,
} from "@/views/diagnostics/serial-reader-status";

type TriggerRow = TriggerRegistrationStatus & {
  activity: TriggerDispatchActivity | null;
  scriptId: string;
  scriptName: string;
};

type TriggerSortColumn = "health" | "node" | "script" | "target" | "type";

const triggerSortSelectors: Record<
  TriggerSortColumn,
  (row: TriggerRow) => number | string
> = {
  health: (row) => {
    if (!row.activity || row.activity.total_dispatch_count === 0) return 0;
    return row.activity.failed_dispatch_count > 0 ? 2 : 1;
  },
  node: (row) => row.node_id,
  script: (row) => row.scriptName,
  target: (row) => row.target ?? row.device_id ?? "",
  type: (row) => triggerDisplayName(row.runner_type),
};

export function TriggerRegistrationPanel({ dashboard }: { dashboard: DashboardPayload }) {
  const rows = triggerRows(dashboard);
  const { sortedRows, sortState, toggleSort } = useSortableRows(
    rows,
    triggerSortSelectors,
  );
  const serialRows = rows.filter(
    (row): row is TriggerRow & SerialTriggerRegistration => row.runner_type === "serial_input",
  );

  return (
    <Card>
      <CardHeader className="flex flex-row items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <ListTree className="size-4 text-muted-foreground" />
            <CardTitle>Registered triggers</CardTitle>
          </div>
          <p className="mt-1 text-xs text-muted-foreground">
            Triggers loaded automatically from enabled and approved scripts.
            Run totals cover the current background runner session and reset when it restarts.
          </p>
        </div>
        <Badge variant="muted">{rows.length}</Badge>
      </CardHeader>
      <CardContent className="overflow-x-auto p-0 max-[1280px]:p-3">
        {rows.length === 0 ? (
          <div className="p-3 text-sm text-muted-foreground max-[1280px]:p-0">
            No trigger registrations are currently loaded.
          </div>
        ) : (
          <table className="responsive-table w-full border-collapse text-sm">
            <thead>
              <tr className="border-b border-border text-left text-xs uppercase text-muted-foreground">
                <SortableTableHeader column="script" onSort={toggleSort} sortState={sortState}>
                  Script
                </SortableTableHeader>
                <SortableTableHeader column="type" onSort={toggleSort} sortState={sortState}>
                  Type
                </SortableTableHeader>
                <SortableTableHeader column="node" onSort={toggleSort} sortState={sortState}>
                  Node
                </SortableTableHeader>
                <SortableTableHeader column="health" onSort={toggleSort} sortState={sortState}>
                  Health
                </SortableTableHeader>
                <SortableTableHeader column="target" onSort={toggleSort} sortState={sortState}>
                  Target
                </SortableTableHeader>
              </tr>
            </thead>
            <tbody>
              {sortedRows.map((row) => (
                <tr
                  className="border-b border-border last:border-b-0"
                  key={`${row.scriptId}:${row.node_id}`}
                >
                  <td className="px-3 py-3" data-label="Script">
                    <div className="font-medium">{row.scriptName}</div>
                    <div className="break-all font-mono text-xs text-muted-foreground">
                      {row.scriptId}
                    </div>
                  </td>
                  <td className="px-3 py-3" data-label="Type">
                    <Badge variant="muted">{triggerDisplayName(row.runner_type)}</Badge>
                  </td>
                  <td className="break-all px-3 py-3 font-mono text-xs" data-label="Node">
                    {row.node_id}
                  </td>
                  <td className="px-3 py-3" data-label="Health">
                    <TriggerHealth activity={row.activity} />
                  </td>
                  <td className="break-words px-3 py-3 font-mono text-xs" data-label="Target">
                    {row.target ?? row.device_id ?? "Not applicable"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {serialRows.length > 0 ? (
          <SerialReaderStatusPanel dashboard={dashboard} registrations={serialRows} />
        ) : null}
      </CardContent>
    </Card>
  );
}

function triggerRows(dashboard: DashboardPayload): TriggerRow[] {
  const activityByTrigger = dashboard.service_status?.activity?.triggers ?? {};
  return dashboard.runner.scripts
    .flatMap((script) =>
      script.triggers.map((trigger) => ({
        ...trigger,
        activity:
          activityByTrigger[`${script.installed.id}::${trigger.node_id}`] ?? null,
        scriptId: script.installed.id,
        scriptName: script.installed.name,
      })),
    )
    .sort(
      (left, right) =>
        left.scriptName.localeCompare(right.scriptName) ||
        left.node_id.localeCompare(right.node_id),
    );
}

function TriggerHealth({ activity }: { activity: TriggerDispatchActivity | null }) {
  const { formatUnixSeconds } = useDesktopTime();
  if (!activity || activity.total_dispatch_count === 0) {
    return (
      <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
        <Activity className="size-3.5" />
        No runs
      </div>
    );
  }

  const failed = activity.failed_dispatch_count > 0;
  return (
    <div className="grid gap-1 text-xs">
      <div className="flex flex-wrap items-center gap-1.5">
        <Badge variant={failed ? "medium" : "good"}>
          {activity.successful_dispatch_count}/{activity.total_dispatch_count} successful
        </Badge>
        {failed ? (
          <Badge variant="destructive">{activity.failed_dispatch_count} failed</Badge>
        ) : null}
      </div>
      {activity.last_dispatch ? (
        <span className="text-muted-foreground">
          Last {activity.last_dispatch.status} at{" "}
          {formatUnixSeconds(activity.last_dispatch.completed_at_unix)}
        </span>
      ) : null}
    </div>
  );
}

function triggerDisplayName(value: string) {
  return value.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
}

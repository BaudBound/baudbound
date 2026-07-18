import { Activity, ListTree } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type {
  DashboardPayload,
  TriggerDispatchActivity,
  TriggerRegistrationStatus,
} from "@/lib/runner-api";
import { useDesktopTime } from "@/lib/time-format";
import {
  SerialReaderStatusPanel,
  type SerialTriggerRegistration,
} from "@/views/diagnostics/serial-reader-status";

type TriggerRow = TriggerRegistrationStatus & {
  activity: TriggerDispatchActivity | null;
  scriptId: string;
  scriptName: string;
};

export function TriggerRegistrationPanel({ dashboard }: { dashboard: DashboardPayload }) {
  const rows = triggerRows(dashboard);
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
                <th className="px-3 py-2">Script</th>
                <th className="px-3 py-2">Type</th>
                <th className="px-3 py-2">Node</th>
                <th className="px-3 py-2">Health</th>
                <th className="px-3 py-2">Target</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
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

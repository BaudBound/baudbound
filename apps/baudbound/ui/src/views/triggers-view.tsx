import { Activity } from "lucide-react";
import { useMemo } from "react";

import { EmptyState } from "@/components/empty-state";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  type DashboardPayload,
  type TriggerDispatchActivity,
  type TriggerRegistrationStatus,
} from "@/lib/runner-api";
import { useDesktopTime } from "@/lib/time-format";
import { SerialReaderStatusPanel } from "@/views/triggers/serial-reader-status";

type TriggerRow = TriggerRegistrationStatus & {
  activity: TriggerDispatchActivity | null;
  scriptId: string;
  scriptName: string;
};

export function TriggersView({ dashboard }: { dashboard: DashboardPayload }) {
  const rows = useMemo(() => triggerRows(dashboard), [dashboard]);
  const grouped = useMemo(() => groupTriggers(rows), [rows]);

  return (
    <div className="grid gap-4">
      <Card>
        <CardContent>
          <div>
            <div className="text-sm font-medium">Trigger registrations</div>
            <div className="text-xs text-muted-foreground">
              Registrations from enabled and approved scripts are loaded automatically by the
              desktop background runner.
            </div>
          </div>
        </CardContent>
      </Card>

      {rows.length === 0 ? (
        <EmptyState>No trigger registrations are available.</EmptyState>
      ) : (
        <div className="grid gap-4">
          {grouped.map((group) => (
            <Card key={group.name}>
              <CardHeader className="flex flex-row items-center justify-between gap-3">
                <CardTitle>{triggerDisplayName(group.name)}</CardTitle>
                <Badge variant="default">{group.rows.length}</Badge>
              </CardHeader>
              <CardContent className="p-0">
                <table className="responsive-table w-full border-collapse text-sm">
                  <thead>
                    <tr className="border-b border-border text-left text-xs uppercase text-muted-foreground">
                      <th className="px-3 py-2">Script</th>
                      <th className="px-3 py-2">Node</th>
                      <th className="px-3 py-2">Runner</th>
                      <th className="px-3 py-2">Health</th>
                      <th className="px-3 py-2">Target</th>
                      <th className="px-3 py-2">Action</th>
                    </tr>
                  </thead>
                  <tbody>
                    {group.rows.map((row) => (
                      <tr
                        className="border-b border-border last:border-b-0"
                        key={`${row.scriptId}-${row.node_id}`}
                      >
                        <td className="px-3 py-3" data-label="Script">
                          <div className="font-medium">{row.scriptName}</div>
                          <div className="font-mono text-xs text-muted-foreground">
                            {row.scriptId}
                          </div>
                        </td>
                        <td className="px-3 py-3 font-mono text-xs" data-label="Node">
                          {row.node_id}
                        </td>
                        <td className="px-3 py-3" data-label="Runner">
                          <Badge variant="muted">{row.runner_type}</Badge>
                        </td>
                        <td className="px-3 py-3" data-label="Health">
                          <TriggerHealth activity={row.activity} />
                        </td>
                        <td className="px-3 py-3" data-label="Target">
                          <span className="break-words font-mono text-xs">
                            {row.target ?? "-"}
                          </span>
                        </td>
                        <td className="px-3 py-3" data-label="Action">
                          {row.action_type}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                {group.name === "serial_input" ? (
                  <SerialReaderStatusPanel
                    dashboard={dashboard}
                    registrations={group.rows}
                  />
                ) : null}
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}

function triggerRows(dashboard: DashboardPayload): TriggerRow[] {
  const activityByTrigger = dashboard.service_status?.activity?.triggers ?? {};
  return (dashboard.runner.scripts ?? []).flatMap((script) =>
    (script.triggers ?? []).map((trigger) => ({
      ...trigger,
      activity: activityByTrigger[triggerActivityKey(script.installed.id, trigger.node_id)] ?? null,
      scriptId: script.installed.id,
      scriptName: script.installed.name,
    })),
  );
}

function groupTriggers(rows: TriggerRow[]) {
  const groups = new Map<string, TriggerRow[]>();
  for (const row of rows) {
    const key = row.runner_type || row.action_type || "unknown";
    groups.set(key, [...(groups.get(key) ?? []), row]);
  }
  return Array.from(groups.entries())
    .map(([name, groupRows]) => ({ name, rows: groupRows }))
    .sort((a, b) => a.name.localeCompare(b.name));
}

function triggerDisplayName(name: string) {
  return name.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
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
  const lastDispatch = activity.last_dispatch;
  return (
    <div className="grid gap-1 text-xs">
      <div className="flex flex-wrap items-center gap-1.5">
        <Badge variant={failed ? "medium" : "good"}>
          {activity.successful_dispatch_count}/{activity.total_dispatch_count} ok
        </Badge>
        {failed ? <Badge variant="destructive">{activity.failed_dispatch_count} failed</Badge> : null}
      </div>
      {lastDispatch ? (
        <div className="text-muted-foreground">
          Last {lastDispatch.status} at {formatUnixSeconds(lastDispatch.completed_at_unix)}
        </div>
      ) : null}
    </div>
  );
}

function triggerActivityKey(scriptId: string, nodeId: string) {
  return `${scriptId}::${nodeId}`;
}

import { KeyRound } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { DashboardAction } from "@/lib/app-types";
import type {
  DashboardPayload,
  NetworkTriggerType,
  TriggerAuthStatus,
} from "@/lib/runner-api";
import { isApprovalCurrent } from "@/lib/status-format";
import { useDesktopTime } from "@/lib/time-format";
import { NetworkTriggerAuthControls } from "@/views/security/network-trigger-auth";

export type NetworkTriggerAuthRow = {
  approvalCurrent: boolean;
  auth: TriggerAuthStatus | null;
  nodeId: string;
  scriptId: string;
  scriptName: string;
  triggerType: NetworkTriggerType;
};

export function NetworkTriggerSecurityPanel({
  busyActions,
  dashboard,
  onDashboard,
  runAction,
}: {
  busyActions: Set<string>;
  dashboard: DashboardPayload;
  onDashboard: (dashboard: DashboardPayload) => void;
  runAction: DashboardAction;
}) {
  const { formatUnixSeconds } = useDesktopTime();
  const rows = networkTriggerAuthRows(dashboard);

  return (
    <Card>
      <CardHeader className="flex flex-row items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <KeyRound className="size-4 text-muted-foreground" />
            <CardTitle>Network trigger authentication</CardTitle>
          </div>
          <p className="mt-1 text-xs text-muted-foreground">
            Manage access tokens for installed Webhook and WebSocket triggers.
          </p>
        </div>
        <Badge
          variant={rows.every((row) => !row.auth || row.auth.auth_enabled) ? "good" : "destructive"}
        >
          {rows.length} network trigger{rows.length === 1 ? "" : "s"}
        </Badge>
      </CardHeader>
      <CardContent className="p-0 max-[900px]:p-3">
        {rows.length === 0 ? (
          <div className="p-3 text-sm text-muted-foreground max-[900px]:p-0">
            No installed scripts use Webhook or WebSocket triggers.
          </div>
        ) : (
          <table className="responsive-table w-full border-collapse text-sm">
            <thead>
              <tr className="border-b border-border text-left text-xs uppercase text-muted-foreground">
                <th className="px-3 py-2">Script</th>
                <th className="px-3 py-2">Trigger</th>
                <th className="px-3 py-2">Created</th>
                <th className="px-3 py-2">Last rotation</th>
                <th className="px-3 py-2">Authentication</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr
                  className="border-b border-border last:border-b-0"
                  key={`${row.scriptId}:${row.nodeId}:${row.triggerType}`}
                >
                  <td className="px-3 py-3" data-label="Script">
                    <div className="font-medium">{row.scriptName}</div>
                    <div className="break-all font-mono text-xs text-muted-foreground">
                      {row.scriptId}
                    </div>
                  </td>
                  <td className="px-3 py-3" data-label="Trigger">
                    <Badge variant="muted">{triggerTypeLabel(row.triggerType)}</Badge>
                    <div className="mt-1 break-all font-mono text-xs text-muted-foreground">
                      {row.nodeId}
                    </div>
                  </td>
                  <td className="px-3 py-3 text-xs" data-label="Created">
                    {row.auth ? formatUnixSeconds(row.auth.created_at_unix) : "After approval"}
                  </td>
                  <td className="px-3 py-3 text-xs" data-label="Last rotation">
                    {row.auth?.rotated_at_unix
                      ? formatUnixSeconds(row.auth.rotated_at_unix)
                      : "Never"}
                  </td>
                  <td className="px-3 py-3" data-label="Authentication">
                    {row.auth ? (
                      <NetworkTriggerAuthControls
                        auth={row.auth}
                        busyActions={busyActions}
                        onDashboard={onDashboard}
                        runAction={runAction}
                      />
                    ) : (
                      <div className="grid gap-1">
                        <Badge variant={row.approvalCurrent ? "destructive" : "medium"}>
                          {row.approvalCurrent ? "Unavailable" : "Awaiting approval"}
                        </Badge>
                        <span className="text-xs text-muted-foreground">
                          {row.approvalCurrent
                            ? "Authentication state could not be created."
                            : "Approve this package to create its token."}
                        </span>
                      </div>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </CardContent>
    </Card>
  );
}

export function networkTriggerAuthRows(dashboard: DashboardPayload): NetworkTriggerAuthRow[] {
  return dashboard.runner.scripts
    .flatMap((script) => {
      const authByTrigger = new Map(
        (dashboard.trigger_auth_statuses[script.installed.id] ?? []).map((auth) => [
          `${auth.node_id}:${auth.trigger_type}`,
          auth,
        ]),
      );
      return script.triggers.flatMap((trigger) => {
        const triggerType = networkTriggerType(trigger.runner_type);
        if (!triggerType) return [];
        return [{
          approvalCurrent: isApprovalCurrent(script.approval_status),
          auth: authByTrigger.get(`${trigger.node_id}:${triggerType}`) ?? null,
          nodeId: trigger.node_id,
          scriptId: script.installed.id,
          scriptName: script.installed.name,
          triggerType,
        }];
      });
    })
    .sort(
      (left, right) =>
        left.scriptName.localeCompare(right.scriptName) ||
        left.nodeId.localeCompare(right.nodeId),
    );
}

function networkTriggerType(runnerType: string): NetworkTriggerType | null {
  if (runnerType === "webhook" || runnerType === "websocket") return runnerType;
  return null;
}

function triggerTypeLabel(triggerType: TriggerAuthStatus["trigger_type"]) {
  return triggerType === "websocket" ? "WebSocket" : "Webhook";
}

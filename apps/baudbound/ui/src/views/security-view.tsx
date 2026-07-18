import { ShieldCheck, ShieldAlert } from "lucide-react";

import { EmptyState } from "@/components/empty-state";
import { StatusSummaryCard } from "@/components/status-summary-card";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { DashboardPayload, ScriptStatus } from "@/lib/runner-api";
import type { DashboardAction } from "@/lib/app-types";
import { approvalLabel, approvalVariant, isApprovalCurrent } from "@/lib/status-format";
import { SecretManagementPanel } from "@/views/secret-management-panel";
import { NetworkTriggerSecurityPanel } from "@/views/security/network-trigger-security-panel";

export function SecurityView({
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
  const scripts = dashboard.runner.scripts;
  const attention = scripts.filter(scriptNeedsAttention);
  const networkAuth = Object.values(dashboard.trigger_auth_statuses).flat();
  const unprotectedNetworkTriggers = networkAuth.filter((auth) => !auth.auth_enabled).length;

  return (
    <div className="grid gap-4">
      <div className="status-summary-grid grid min-w-0 gap-3">
        <StatusSummaryCard label="Installed" value={scripts.length} />
        <StatusSummaryCard label="Needs attention" tone="medium" value={attention.length} />
        <StatusSummaryCard
          label="Approved"
          tone="good"
          value={scripts.filter((script) => isApprovalCurrent(script.approval_status)).length}
        />
        <StatusSummaryCard
          badgeLabel={unprotectedNetworkTriggers > 0 ? "Review" : "Protected"}
          label="Unprotected"
          tone={unprotectedNetworkTriggers > 0 ? "destructive" : "good"}
          value={unprotectedNetworkTriggers}
        />
        <StatusSummaryCard
          label="High risk"
          tone="destructive"
          value={scripts.filter((script) => script.installed.risk_level === "high").length}
        />
      </div>

      {scripts.length === 0 ? (
        <EmptyState>No scripts are installed.</EmptyState>
      ) : (
        <Card>
          <CardHeader>
            <CardTitle>Script security review</CardTitle>
          </CardHeader>
          <CardContent className="overflow-x-auto p-0 max-[1280px]:p-3">
            <table className="responsive-table w-full border-collapse text-sm">
              <thead>
                <tr className="border-b border-border text-left text-xs uppercase text-muted-foreground">
                  <th className="px-3 py-2">Script</th>
                  <th className="px-3 py-2">Approval</th>
                  <th className="px-3 py-2">Risk</th>
                  <th className="px-3 py-2">Permissions</th>
                  <th className="px-3 py-2">Package</th>
                  <th className="px-3 py-2">Issues</th>
                </tr>
              </thead>
              <tbody>
                {scripts.map((script) => (
                  <tr className="border-b border-border last:border-b-0" key={script.installed.id}>
                    <td className="px-3 py-3" data-label="Script">
                      <div className="font-medium">{script.installed.name}</div>
                      <div className="font-mono text-xs text-muted-foreground">
                        {script.installed.id}
                      </div>
                    </td>
                    <td className="px-3 py-3" data-label="Approval">
                      <Badge variant={approvalVariant(script.approval_status)}>
                        {approvalLabel(script.approval_status)}
                      </Badge>
                    </td>
                    <td className="px-3 py-3" data-label="Risk">
                      <Badge variant={riskVariant(script.installed.risk_level)}>
                        {script.installed.risk_level}
                      </Badge>
                    </td>
                    <td className="px-3 py-3" data-label="Permissions">
                      {script.declared_permissions.length > 0 ? (
                        <div className="flex max-w-[320px] flex-wrap gap-1">
                          {script.declared_permissions.map((permission) => (
                            <Badge key={permission} variant="muted">
                              {permission}
                            </Badge>
                          ))}
                        </div>
                      ) : (
                        <span className="text-muted-foreground">None declared</span>
                      )}
                    </td>
                    <td className="px-3 py-3" data-label="Package">
                      <div>{script.installed.package_file_name}</div>
                      <div className="font-mono text-xs text-muted-foreground">
                        {script.installed.package_hash.slice(0, 16)}...
                      </div>
                    </td>
                    <td className="max-w-[360px] px-3 py-3" data-label="Issues">
                      {securityIssue(script) ? (
                        <div className="flex gap-2 text-baud-amber">
                          <ShieldAlert className="mt-0.5 size-4 shrink-0" />
                          <span>{securityIssue(script)}</span>
                        </div>
                      ) : (
                        <div className="flex gap-2 text-baud-green">
                          <ShieldCheck className="mt-0.5 size-4 shrink-0" />
                          <span>No active security issues.</span>
                        </div>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </CardContent>
        </Card>
      )}

      <NetworkTriggerSecurityPanel
        busyActions={busyActions}
        dashboard={dashboard}
        onDashboard={onDashboard}
        runAction={runAction}
      />

      <SecretManagementPanel
        busyActions={busyActions}
        dashboard={dashboard}
        runAction={runAction}
      />
    </div>
  );
}

function scriptNeedsAttention(script: ScriptStatus) {
  return !isApprovalCurrent(script.approval_status) || Boolean(script.package_error);
}

function securityIssue(script: ScriptStatus) {
  if (script.package_error) return script.package_error;
  if (!isApprovalCurrent(script.approval_status)) {
    return `Approval is ${approvalLabel(script.approval_status).toLowerCase()}.`;
  }
  return null;
}

function riskVariant(risk: string) {
  if (risk === "high") return "destructive";
  if (risk === "medium") return "medium";
  if (risk === "low") return "good";
  return "muted";
}

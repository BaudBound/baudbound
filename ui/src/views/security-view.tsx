import { ExternalLink, RefreshCw, ShieldAlert, ShieldCheck } from "lucide-react";

import { EmptyState } from "@/components/empty-state";
import { StatusSummaryCard } from "@/components/status-summary-card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { SortableTableHeader } from "@/components/ui/sortable-table-header";
import type { DashboardAction } from "@/lib/app-types";
import { openExternalUrl } from "@/lib/external-url";
import {
  type BlacklistSeverity,
  checkOfficialBlacklist,
  type DashboardPayload,
  type ScriptStatus,
} from "@/lib/runner-api";
import { approvalIssueDescription, approvalLabel, approvalVariant, isApprovalCurrent } from "@/lib/status-format";
import { useSortableRows } from "@/lib/table-sorting";
import { useDesktopTime } from "@/lib/time-format";
import { SecretManagementPanel } from "@/views/secret-management-panel";
import { NetworkTriggerSecurityPanel } from "@/views/security/network-trigger-security-panel";

type SecuritySortColumn = "approval" | "issues" | "package" | "permissions" | "risk" | "script";

const securitySortSelectors: Record<SecuritySortColumn, (script: ScriptStatus) => number | string> = {
  approval: (script) => approvalLabel(script.approval_status),
  issues: (script) => securityIssue(script) ?? "",
  package: (script) => script.installed.package_file_name,
  permissions: (script) => script.declared_permissions.join("\n"),
  risk: (script) => riskOrder(script.installed.risk_level),
  script: (script) => script.installed.name,
};

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
  const { formatUnixSeconds } = useDesktopTime();
  const { sortedRows: sortedScripts, sortState, toggleSort } = useSortableRows(scripts, securitySortSelectors);
  const attention = scripts.filter((script) => script.installed.enabled && scriptNeedsAttention(script));
  const networkAuth = Object.values(dashboard.trigger_auth_statuses).flat();
  const unprotectedNetworkTriggers = networkAuth.filter((auth) => !auth.auth_enabled).length;
  const blacklist = dashboard.blacklist;
  const incidentEntries = blacklist.incidents
    .map((incident) => ({
      entry: blacklist.entries.find((entry) => entry.id === incident.entry_id),
      incident,
    }))
    .filter((value) => value.entry);

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
            <table className="responsive-table min-w-[980px] w-full border-collapse text-sm max-[1280px]:min-w-0">
              <thead>
                <tr className="border-b border-border text-left text-xs uppercase text-muted-foreground">
                  <SortableTableHeader column="script" onSort={toggleSort} sortState={sortState}>
                    Script
                  </SortableTableHeader>
                  <SortableTableHeader column="approval" onSort={toggleSort} sortState={sortState}>
                    Approval
                  </SortableTableHeader>
                  <SortableTableHeader column="risk" onSort={toggleSort} sortState={sortState}>
                    Risk
                  </SortableTableHeader>
                  <SortableTableHeader column="permissions" onSort={toggleSort} sortState={sortState}>
                    Permissions
                  </SortableTableHeader>
                  <SortableTableHeader column="package" onSort={toggleSort} sortState={sortState}>
                    Package
                  </SortableTableHeader>
                  <SortableTableHeader column="issues" onSort={toggleSort} sortState={sortState}>
                    Issues
                  </SortableTableHeader>
                </tr>
              </thead>
              <tbody>
                {sortedScripts.map((script) => (
                  <tr className="border-b border-border last:border-b-0" key={script.installed.id}>
                    <td className="px-3 py-3" data-label="Script">
                      <div className="font-medium">{script.installed.name}</div>
                      <div className="font-mono text-xs text-muted-foreground">{script.installed.id}</div>
                    </td>
                    <td className="px-3 py-3" data-label="Approval">
                      <Badge variant={approvalVariant(script.approval_status)}>
                        {approvalLabel(script.approval_status)}
                      </Badge>
                    </td>
                    <td className="px-3 py-3" data-label="Risk">
                      <Badge variant={riskVariant(script.installed.risk_level)}>{script.installed.risk_level}</Badge>
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

      <Card>
        <CardHeader className="flex-row items-start justify-between gap-3">
          <div className="min-w-0">
            <CardTitle>Blacklist</CardTitle>
            <p className="mt-1 text-sm text-muted-foreground">
              {blacklist.fetched_at_unix
                ? `Last checked ${formatUnixSeconds(blacklist.fetched_at_unix)}`
                : "No blacklist has been downloaded yet."}
            </p>
          </div>
          <Button
            disabled={busyActions.has("check-blacklist")}
            onClick={() => void runAction("check-blacklist", checkOfficialBlacklist)}
            size="sm"
            variant="outline"
          >
            <RefreshCw className={busyActions.has("check-blacklist") ? "animate-spin" : ""} />
            Check now
          </Button>
        </CardHeader>
        <CardContent className="grid gap-3">
          <div className="flex flex-wrap gap-2">
            <Badge variant={blacklist.api_available ? "good" : "medium"}>
              {blacklist.api_available ? "API available" : "Using cached data"}
            </Badge>
            <Badge variant={blacklist.stale ? "medium" : "muted"}>
              {blacklist.stale ? "Cache stale" : "Cache current"}
            </Badge>
            <Badge variant="muted">{blacklist.active_entry_count} active entries</Badge>
          </div>
          {blacklist.last_error ? <p className="text-sm text-baud-amber">{blacklist.last_error}</p> : null}
          {incidentEntries.length === 0 ? (
            <EmptyState>No installed scripts are affected by the blacklist.</EmptyState>
          ) : (
            <div className="grid gap-2">
              {incidentEntries.map(({ entry, incident }) => (
                <div
                  className="grid gap-2 rounded-md border border-border px-3 py-2 text-sm sm:grid-cols-[minmax(0,1fr)_auto]"
                  key={`${incident.entry_id}-${incident.script_id ?? incident.repository_url ?? "repository"}`}
                >
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="font-medium">{entry?.title ?? incident.title}</span>
                      <Badge variant={blacklistVariant(incident.severity)}>{blacklistLabel(incident.severity)}</Badge>
                    </div>
                    <p className="mt-1 text-muted-foreground">{entry?.reason ?? incident.reason}</p>
                    {entry ? (
                      <div className="mt-1 flex flex-wrap gap-x-3 gap-y-1 text-xs text-muted-foreground">
                        <span>Scope {entry.scope}</span>
                        <span>Published {entry.published_at}</span>
                        {entry.scope === "domain" ? (
                          <span>{entry.subdomains ? "Includes subdomains" : "Exact domain only"}</span>
                        ) : null}
                        {entry.scope === "publisher" ? <span>{entry.target}</span> : null}
                      </div>
                    ) : null}
                    {incident.script_id ? (
                      <p className="mt-1 font-mono text-xs text-muted-foreground">Script {incident.script_id}</p>
                    ) : null}
                    {incident.repository_url ? (
                      <p className="mt-1 break-all font-mono text-xs text-muted-foreground">
                        Repository {incident.repository_url}
                      </p>
                    ) : null}
                  </div>
                  {entry?.advisory_url || incident.advisory_url ? (
                    <Button
                      onClick={() => void openExternalUrl(entry?.advisory_url ?? incident.advisory_url)}
                      size="sm"
                      variant="outline"
                    >
                      <ExternalLink />
                      Advisory
                    </Button>
                  ) : null}
                </div>
              ))}
            </div>
          )}
          <p className="text-xs text-muted-foreground">
            Low entries are advisories. Medium entries block distribution. High entries quarantine installed scripts.
            Critical entries also request cancellation of active runs.
          </p>
        </CardContent>
      </Card>

      <NetworkTriggerSecurityPanel
        busyActions={busyActions}
        dashboard={dashboard}
        onDashboard={onDashboard}
        runAction={runAction}
      />

      <SecretManagementPanel busyActions={busyActions} dashboard={dashboard} runAction={runAction} />
    </div>
  );
}

function blacklistLabel(severity: BlacklistSeverity) {
  if (severity === "critical") return "Critical quarantine";
  if (severity === "high") return "Quarantined";
  if (severity === "medium") return "Restricted";
  return "Advisory";
}

function blacklistVariant(severity: BlacklistSeverity) {
  if (severity === "critical" || severity === "high") return "destructive";
  if (severity === "medium") return "medium";
  return "muted";
}

function scriptNeedsAttention(script: ScriptStatus) {
  return !isApprovalCurrent(script.approval_status) || Boolean(script.package_error);
}

function securityIssue(script: ScriptStatus) {
  if (script.package_error) return script.package_error;
  return approvalIssueDescription(script.approval_status);
}

function riskVariant(risk: string) {
  if (risk === "high") return "destructive";
  if (risk === "medium") return "medium";
  if (risk === "low") return "good";
  return "muted";
}

function riskOrder(risk: string) {
  if (risk === "low") return 0;
  if (risk === "medium") return 1;
  if (risk === "high") return 2;
  return 3;
}

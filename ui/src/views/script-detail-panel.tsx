import { Eye, RefreshCw } from "lucide-react";
import { useState } from "react";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { Details } from "@/components/details";
import { EmptyState } from "@/components/empty-state";
import { ExternalLink } from "@/components/external-link";
import { LazyMarkdownContent } from "@/components/lazy-markdown-content";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { SortableTableHeader } from "@/components/ui/sortable-table-header";
import { Switch } from "@/components/ui/switch";
import type { DashboardAction } from "@/lib/app-types";
import {
  checkScriptUpdate,
  type ScriptStatus,
  type ScriptUpdateState,
  setScriptAutomaticUpdateChecks,
  type StoredRunRecord,
} from "@/lib/runner-api";
import { approvalLabel, isApprovalCurrent, packageHashLabel, riskVariant } from "@/lib/status-format";
import { useDesktopTime } from "@/lib/time-format";
import { useSortableRows } from "@/lib/table-sorting";
import { RemotePackageDialog } from "@/views/remote-package-dialog";

type TriggerSortColumn = "action" | "node" | "runnerType";
type RecentRunSortColumn = "completed" | "runId" | "status" | "trigger";

const triggerSortSelectors: Record<
  TriggerSortColumn,
  (trigger: ScriptStatus["triggers"][number]) => string
> = {
  action: (trigger) => trigger.action_type,
  node: (trigger) => trigger.node_id,
  runnerType: (trigger) => trigger.runner_type,
};

const recentRunSortSelectors: Record<
  RecentRunSortColumn,
  (run: StoredRunRecord) => number | string
> = {
  completed: (run) => run.completed_at_unix,
  runId: (run) => run.run_id,
  status: (run) => run.status,
  trigger: (run) => run.trigger_node_id,
};

export function ScriptDetailPanel({
  busyActions,
  onViewRun,
  recentRuns,
  runAction,
  script,
  updateState,
}: {
  busyActions: Set<string>;
  onViewRun: (run: StoredRunRecord) => void;
  recentRuns: StoredRunRecord[];
  runAction: DashboardAction;
  script: ScriptStatus;
  updateState: ScriptUpdateState;
}) {
  const [enableChecksOpen, setEnableChecksOpen] = useState(false);
  const [reviewUpdateOpen, setReviewUpdateOpen] = useState(false);
  const { formatUnixSeconds } = useDesktopTime();
  const metadata = script.metadata;
  const scriptRuns = recentRuns
    .filter((run) => run.script_id === script.installed.id)
    .slice(0, 5);
  const {
    sortedRows: sortedTriggers,
    sortState: triggerSortState,
    toggleSort: toggleTriggerSort,
  } = useSortableRows(script.triggers, triggerSortSelectors);
  const {
    sortedRows: sortedRuns,
    sortState: runSortState,
    toggleSort: toggleRunSort,
  } = useSortableRows(scriptRuns, recentRunSortSelectors);

  return (
    <div className="grid gap-4">
      <Card>
        <CardHeader>
          <CardTitle>About this script</CardTitle>
        </CardHeader>
        <CardContent>
          {metadata ? (
            <div className="grid gap-4 text-sm">
              {metadata.description.trim() ? (
                <p className="select-text leading-6 text-foreground">
                  {metadata.description}
                </p>
              ) : null}

              <MetadataRows
                rows={[
                  ["Author", metadata.author],
                  ["Created with", metadata.created_with],
                  ["Created", metadata.created_at],
                  ["Updated", metadata.updated_at],
                  ["Version", metadata.version],
                  ["Repository URL", metadata.repository_url || "Not configured"],
                  ["Minimum runner", metadata.minimum_runner_version],
                ]}
              />

              {metadata.website.trim() || metadata.source.trim() ? (
                <section className="grid gap-2">
                  <h3 className="font-medium">Links</h3>
                  {metadata.website.trim() ? (
                    <div className="grid grid-cols-[6rem_minmax(0,1fr)] gap-3">
                      <span className="text-muted-foreground">Website</span>
                      <ExternalLink href={metadata.website}>
                        {metadata.website}
                      </ExternalLink>
                    </div>
                  ) : null}
                  {metadata.source.trim() ? (
                    <div className="grid grid-cols-[6rem_minmax(0,1fr)] gap-3">
                      <span className="text-muted-foreground">Source</span>
                      <ExternalLink href={metadata.source}>
                        {metadata.source}
                      </ExternalLink>
                    </div>
                  ) : null}
                </section>
              ) : null}

              {metadata.tags.length > 0 ? (
                <section className="grid gap-2">
                  <h3 className="font-medium">Tags</h3>
                  <div className="flex flex-wrap gap-1.5">
                    {metadata.tags.map((tag) => (
                      <Badge key={tag} variant="muted">
                        {tag}
                      </Badge>
                    ))}
                  </div>
                </section>
              ) : null}
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">
              Package information is unavailable because the installed package could not be read and verified.
            </p>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Updates</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-4">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div className="grid gap-1">
              <div className="flex flex-wrap items-center gap-2">
                <span className="text-sm font-medium">Update status</span>
                <Badge variant={updateStatusVariant(updateState.status)}>
                  {updateStatusLabel(updateState.status)}
                </Badge>
              </div>
              <p className="text-xs text-muted-foreground">
                This check only discovers published packages. BaudBound never installs or approves
                a script update automatically.
              </p>
            </div>
            <div className="flex flex-wrap gap-2">
              {updateState.status === "available" ? (
                <Button onClick={() => setReviewUpdateOpen(true)} size="sm">
                  Review update
                </Button>
              ) : null}
              <Button
                disabled={
                  !metadata?.repository_url.trim() ||
                  busyActions.has(`check-script-update:${script.installed.id}`)
                }
                onClick={() =>
                  runAction(`check-script-update:${script.installed.id}`, () =>
                    checkScriptUpdate(script.installed.id),
                  )
                }
                size="sm"
                variant="outline"
              >
                <RefreshCw />
                {busyActions.has(`check-script-update:${script.installed.id}`)
                  ? "Checking..."
                  : "Check for updates"}
              </Button>
            </div>
          </div>

          {metadata?.repository_url.trim() ? (
            <div className="grid gap-2 text-sm">
              <div className="grid grid-cols-[8rem_minmax(0,1fr)] gap-3">
                <span className="text-muted-foreground">Repository URL</span>
                <ExternalLink href={metadata.repository_url}>
                  {metadata.repository_url}
                </ExternalLink>
              </div>
              <MetadataRows
                rows={[
                  ["Current version", metadata.version],
                  ["Latest version", updateState.latest_version ?? "Not checked"],
                  [
                    "Package size",
                    updateState.package_size === null
                      ? "Not available"
                      : formatBytes(updateState.package_size),
                  ],
                  ["Published", updateState.published_at ?? "Not available"],
                  ["Last error", updateState.last_error ?? ""],
                ]}
              />
              {updateState.release_notes?.trim() ? (
                <section className="grid gap-2 border-t border-border pt-3">
                  <h3 className="font-medium">Release notes</h3>
                  <LazyMarkdownContent source={updateState.release_notes} />
                </section>
              ) : null}
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">
              This script does not provide a repository URL.
            </p>
          )}

          <div className="flex items-start justify-between gap-4 border-t border-border pt-4">
            <div>
              <div className="text-sm font-medium">Automatic update checks</div>
              <p className="text-xs text-muted-foreground">
                Periodically contact this script publisher to discover new versions.
              </p>
            </div>
            <Switch
              checked={updateState.automatic_checks_enabled}
              disabled={
                !metadata?.repository_url.trim() ||
                busyActions.has(`automatic-script-updates:${script.installed.id}`)
              }
              onCheckedChange={(enabled) => {
                if (enabled) {
                  setEnableChecksOpen(true);
                  return;
                }
                void runAction(`automatic-script-updates:${script.installed.id}`, () =>
                  setScriptAutomaticUpdateChecks(script.installed.id, false),
                );
              }}
            />
          </div>
          <ConfirmDialog
            confirmLabel="Enable checks"
            description="BaudBound will periodically contact the repository selected by this script publisher. The server can observe your IP address and the time of each check. Updates will only be discovered. They will not be downloaded, installed, enabled, or approved automatically."
            disabled={busyActions.has(`automatic-script-updates:${script.installed.id}`)}
            onConfirm={async () => {
              await runAction(`automatic-script-updates:${script.installed.id}`, () =>
                setScriptAutomaticUpdateChecks(script.installed.id, true),
              );
            }}
            onOpenChange={setEnableChecksOpen}
            open={enableChecksOpen}
            title="Enable automatic update checks?"
          />
          {reviewUpdateOpen ? (
            <RemotePackageDialog
              busyActions={busyActions}
              discoveredScriptId={script.installed.id}
              onOpenChange={setReviewUpdateOpen}
              open
              operation="update"
              runAction={runAction}
            />
          ) : null}
        </CardContent>
      </Card>

      <div className="grid gap-4 xl:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Package</CardTitle>
          </CardHeader>
          <CardContent>
            <Details
              rows={[
                ["ID", script.installed.id],
                ["Package", script.installed.package_file_name],
                ["Path", script.installed.package_path],
                ["Target runtimes", script.installed.target_runtime],
                ["Risk", script.installed.risk_level],
                ["Assets", script.installed.asset_count.toString()],
                ["Package version", script.installed.package_format_version.toString()],
                ["Runtime version", script.installed.script_language_version.toString()],
                ["Imported", formatUnixSeconds(script.installed.imported_at_unix)],
              ]}
            />
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Health</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="mb-3 flex flex-wrap gap-2">
              <Badge variant={riskVariant(script.installed.risk_level)}>
                {script.installed.risk_level}
              </Badge>
              <Badge variant={script.package_error ? "destructive" : "good"}>
                hash {packageHashLabel(script.package_hash_status)}
              </Badge>
              <Badge variant={isApprovalCurrent(script.approval_status) ? "good" : "medium"}>
                approval {approvalLabel(script.approval_status)}
              </Badge>
              <Badge variant={script.installed.enabled ? "good" : "muted"}>
                {script.installed.enabled ? "enabled" : "disabled"}
              </Badge>
            </div>
            {script.package_error ? (
              <p className="rounded-md border border-destructive/40 bg-destructive/10 p-2 text-sm text-destructive">
                {script.package_error}
              </p>
            ) : null}
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Declared permissions</CardTitle>
        </CardHeader>
        <CardContent>
          {script.declared_permissions.length === 0 ? (
            <p className="text-sm text-muted-foreground">No declared permissions.</p>
          ) : (
            <div className="flex flex-wrap gap-1.5">
              {script.declared_permissions.map((permission) => (
                <Badge key={permission} variant="muted">
                  {permission}
                </Badge>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Triggers</CardTitle>
        </CardHeader>
        <CardContent className="p-0 max-[1280px]:p-3">
          {script.triggers.length === 0 ? (
            <p className="p-4 text-sm text-muted-foreground">
              No active trigger registrations.
            </p>
          ) : (
            <div className="overflow-x-auto rounded-md border border-border p-0 max-[1280px]:border-0 max-[1280px]:p-0">
              <table className="responsive-table w-full border-collapse text-sm">
                <thead>
                  <tr className="border-b border-border text-left text-xs uppercase text-muted-foreground">
                    <SortableTableHeader column="node" onSort={toggleTriggerSort} sortState={triggerSortState}>
                      Node
                    </SortableTableHeader>
                    <SortableTableHeader column="action" onSort={toggleTriggerSort} sortState={triggerSortState}>
                      Action
                    </SortableTableHeader>
                    <SortableTableHeader column="runnerType" onSort={toggleTriggerSort} sortState={triggerSortState}>
                      Runner type
                    </SortableTableHeader>
                  </tr>
                </thead>
                <tbody>
                  {sortedTriggers.map((trigger) => (
                    <tr className="border-b border-border last:border-b-0" key={trigger.node_id}>
                      <td className="px-3 py-2" data-label="Node">
                        {trigger.node_id}
                      </td>
                      <td className="px-3 py-2" data-label="Action">
                        {trigger.action_type}
                      </td>
                      <td className="px-3 py-2" data-label="Runner type">
                        {trigger.runner_type}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Recent runs</CardTitle>
        </CardHeader>
        <CardContent className="p-0 max-[1280px]:p-3">
          {scriptRuns.length === 0 ? (
            <div className="p-4">
              <EmptyState>No recent runs for this script.</EmptyState>
            </div>
          ) : (
            <div className="overflow-x-auto rounded-md border border-border p-0 max-[1280px]:border-0 max-[1280px]:p-0">
              <table className="responsive-table w-full border-collapse text-sm">
                <thead>
                  <tr className="border-b border-border text-left text-xs uppercase text-muted-foreground">
                    <SortableTableHeader column="completed" onSort={toggleRunSort} sortState={runSortState}>
                      Completed
                    </SortableTableHeader>
                    <SortableTableHeader column="trigger" onSort={toggleRunSort} sortState={runSortState}>
                      Trigger
                    </SortableTableHeader>
                    <SortableTableHeader column="status" onSort={toggleRunSort} sortState={runSortState}>
                      Status
                    </SortableTableHeader>
                    <SortableTableHeader column="runId" onSort={toggleRunSort} sortState={runSortState}>
                      Run ID
                    </SortableTableHeader>
                    <th className="w-14 px-3 py-2 text-right">
                      <span className="sr-only">View</span>
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {sortedRuns.map((run) => (
                    <tr className="border-b border-border last:border-b-0" key={run.run_id}>
                      <td className="px-3 py-2" data-label="Completed">
                        {formatUnixSeconds(run.completed_at_unix)}
                      </td>
                      <td className="px-3 py-2" data-label="Trigger">
                        {run.trigger_node_id}
                      </td>
                      <td className="px-3 py-2" data-label="Status">
                        {run.status}
                      </td>
                      <td
                        className="px-3 py-2 font-mono text-xs text-muted-foreground"
                        data-label="Run ID"
                      >
                        {run.run_id}
                      </td>
                      <td className="px-3 py-2 text-right" data-label="View">
                        <Button
                          aria-label={`View details for run ${run.run_id}`}
                          className="size-8 p-0"
                          onClick={() => onViewRun(run)}
                          size="sm"
                          title="View details"
                          variant="outline"
                        >
                          <Eye />
                        </Button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

function updateStatusLabel(status: ScriptUpdateState["status"]) {
  switch (status) {
    case "available": return "Update available";
    case "failed": return "Check failed";
    case "not_checked": return "Not checked";
    case "unavailable": return "Unavailable";
    case "unconfigured": return "Not configured";
    case "up_to_date": return "Up to date";
  }
}

function updateStatusVariant(status: ScriptUpdateState["status"]) {
  if (status === "available") return "medium" as const;
  if (status === "failed" || status === "unavailable") return "destructive" as const;
  if (status === "up_to_date") return "good" as const;
  return "muted" as const;
}

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KiB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MiB`;
}

function MetadataRows({ rows }: { rows: Array<[string, string]> }) {
  const visibleRows = rows.filter(([, value]) => value.trim().length > 0);
  if (visibleRows.length === 0) return null;

  return (
    <dl className="grid grid-cols-[max-content_minmax(0,1fr)] gap-x-4 gap-y-2">
      {visibleRows.map(([label, value]) => (
        <div className="contents" key={label}>
          <dt className="text-muted-foreground">{label}</dt>
          <dd className="min-w-0 select-text break-words">{value}</dd>
        </div>
      ))}
    </dl>
  );
}

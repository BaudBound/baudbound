import { Eye } from "lucide-react";

import { Details } from "@/components/details";
import { EmptyState } from "@/components/empty-state";
import { ExternalLink } from "@/components/external-link";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { SortableTableHeader } from "@/components/ui/sortable-table-header";
import { type ScriptStatus, type StoredRunRecord } from "@/lib/runner-api";
import { approvalLabel, isApprovalCurrent, packageHashLabel, riskVariant } from "@/lib/status-format";
import { useDesktopTime } from "@/lib/time-format";
import { useSortableRows } from "@/lib/table-sorting";

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
  onViewRun,
  recentRuns,
  script,
}: {
  onViewRun: (run: StoredRunRecord) => void;
  recentRuns: StoredRunRecord[];
  script: ScriptStatus;
}) {
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
                  ["Minimum runner", metadata.minimum_runner_version],
                ]}
              />

              {metadata.website.trim() || metadata.repository.trim() ? (
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
                  {metadata.repository.trim() ? (
                    <div className="grid grid-cols-[6rem_minmax(0,1fr)] gap-3">
                      <span className="text-muted-foreground">Repository</span>
                      <ExternalLink href={metadata.repository}>
                        {metadata.repository}
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
                ["Target runtime", script.installed.target_runtime],
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

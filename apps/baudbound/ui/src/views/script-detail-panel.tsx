import { Details } from "@/components/details";
import { EmptyState } from "@/components/empty-state";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { type ScriptStatus, type StoredRunRecord } from "@/lib/runner-api";
import { approvalLabel, isApprovalCurrent, packageHashLabel, riskVariant } from "@/lib/status-format";

export function ScriptDetailPanel({
  recentRuns,
  script,
}: {
  recentRuns: StoredRunRecord[];
  script: ScriptStatus;
}) {
  const scriptRuns = recentRuns
    .filter((run) => run.script_id === script.installed.id)
    .slice(0, 5);

  return (
    <Card>
      <CardContent className="grid gap-4">
        <div className="grid gap-4 xl:grid-cols-2">
          <section>
            <h3 className="mb-2 text-sm font-medium">Package</h3>
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
                ["Imported", formatUnixTime(script.installed.imported_at_unix)],
              ]}
            />
          </section>

          <section>
            <h3 className="mb-2 text-sm font-medium">Health</h3>
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
          </section>
        </div>

        <section>
          <h3 className="mb-2 text-sm font-medium">Declared permissions</h3>
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
        </section>

        <section>
          <h3 className="mb-2 text-sm font-medium">Triggers</h3>
          {script.triggers.length === 0 ? (
            <p className="text-sm text-muted-foreground">No active trigger registrations.</p>
          ) : (
            <div className="rounded-md border border-border p-0 max-[900px]:border-0 max-[900px]:p-0">
              <table className="responsive-table w-full border-collapse text-sm">
                <thead>
                  <tr className="border-b border-border text-left text-xs uppercase text-muted-foreground">
                    <th className="px-3 py-2">Node</th>
                    <th className="px-3 py-2">Action</th>
                    <th className="px-3 py-2">Runner type</th>
                  </tr>
                </thead>
                <tbody>
                  {script.triggers.map((trigger) => (
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
        </section>

        <section>
          <h3 className="mb-2 text-sm font-medium">Recent runs</h3>
          {scriptRuns.length === 0 ? (
            <EmptyState>No recent runs for this script.</EmptyState>
          ) : (
            <div className="rounded-md border border-border p-0 max-[900px]:border-0 max-[900px]:p-0">
              <table className="responsive-table w-full border-collapse text-sm">
                <thead>
                  <tr className="border-b border-border text-left text-xs uppercase text-muted-foreground">
                    <th className="px-3 py-2">Completed</th>
                    <th className="px-3 py-2">Trigger</th>
                    <th className="px-3 py-2">Status</th>
                    <th className="px-3 py-2">Run ID</th>
                  </tr>
                </thead>
                <tbody>
                  {scriptRuns.map((run) => (
                    <tr className="border-b border-border last:border-b-0" key={run.run_id}>
                      <td className="px-3 py-2" data-label="Completed">
                        {formatUnixTime(run.completed_at_unix)}
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
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </section>
      </CardContent>
    </Card>
  );
}

function formatUnixTime(value: number) {
  return new Date(value * 1000).toLocaleString();
}

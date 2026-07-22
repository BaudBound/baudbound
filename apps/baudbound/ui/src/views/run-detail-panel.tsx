import { Details } from "@/components/details";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import type { StoredRunRecord } from "@/lib/runner-api";
import {
  countLogsByLevel,
  filterVariableMetadata,
  nodeActionType,
  runStatusVariant,
  variableRows,
} from "@/lib/run-inspection";
import { useDesktopTime } from "@/lib/time-format";
import { RunLogPanel } from "@/views/run-log-panel";
import { RunVariablePanel } from "@/views/run-variable-panel";

export function RunDetailPanel({
  run,
  scriptName,
}: {
  run: StoredRunRecord;
  scriptName: string;
}) {
  const { formatUnixSeconds } = useDesktopTime();
  const logCounts = countLogsByLevel(run.logs);
  const errorCount = logCounts.error ?? 0;
  const warningCount = (logCounts.warn ?? 0) + (logCounts.warning ?? 0);
  const dataVariableCount = filterVariableMetadata(
    variableRows(run.variables, run.variable_scopes),
    false,
  ).length;

  return (
    <div className="grid gap-4">
      <div className="grid gap-4 xl:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Run</CardTitle>
          </CardHeader>
          <CardContent>
            <Details
              rows={[
                ["Run ID", run.run_id],
                ["Script", scriptName],
                ["Script ID", run.script_id],
                ["Trigger", run.trigger_node_id],
                ["Trigger type", nodeActionType(run.logs, run.trigger_node_id) ?? "Unknown"],
                ["Completed", formatUnixSeconds(run.completed_at_unix)],
              ]}
            />
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Result</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="flex flex-wrap gap-2">
              <Badge variant={runStatusVariant(run.status)}>
                {run.status}
              </Badge>
              <Badge variant="muted">{run.logs.length} log entries</Badge>
              <Badge variant={errorCount > 0 ? "destructive" : "muted"}>
                {errorCount} errors
              </Badge>
              <Badge variant={warningCount > 0 ? "medium" : "muted"}>
                {warningCount} warnings
              </Badge>
              <Badge variant="muted">{dataVariableCount} variables</Badge>
            </div>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Logs</CardTitle>
        </CardHeader>
        <CardContent>
          <RunLogPanel logs={run.logs} />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Variables</CardTitle>
        </CardHeader>
        <CardContent>
          <RunVariablePanel
            variableScopes={run.variable_scopes}
            variables={run.variables}
          />
        </CardContent>
      </Card>
    </div>
  );
}

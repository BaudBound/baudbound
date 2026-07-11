import { Details } from "@/components/details";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import type { StoredRunRecord } from "@/lib/runner-api";
import { runStatusVariant } from "@/lib/run-inspection";
import { countLogsByLevel } from "@/lib/run-inspection";
import { RunLogPanel } from "@/views/run-log-panel";
import { RunVariablePanel } from "@/views/run-variable-panel";

export function RunDetailPanel({
  run,
  scriptName,
}: {
  run: StoredRunRecord;
  scriptName: string;
}) {
  const logCounts = countLogsByLevel(run.logs);
  const errorCount = logCounts.error ?? 0;
  const warningCount = (logCounts.warn ?? 0) + (logCounts.warning ?? 0);

  return (
    <Card>
      <CardContent className="grid gap-4">
        <div className="grid gap-4 xl:grid-cols-2">
          <section>
            <h3 className="mb-2 text-sm font-medium">Run</h3>
            <Details
              rows={[
                ["Run ID", run.run_id],
                ["Script", scriptName],
                ["Script ID", run.script_id],
                ["Trigger", run.trigger_node_id],
                ["Completed", formatUnixTime(run.completed_at_unix)],
              ]}
            />
          </section>

          <section>
            <h3 className="mb-2 text-sm font-medium">Result</h3>
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
              <Badge variant="muted">{Object.keys(run.variables).length} variables</Badge>
            </div>
          </section>
        </div>

        <section>
          <h3 className="mb-2 text-sm font-medium">Logs</h3>
          <RunLogPanel logs={run.logs} />
        </section>

        <section>
          <h3 className="mb-2 text-sm font-medium">Variables</h3>
          <RunVariablePanel variables={run.variables} />
        </section>
      </CardContent>
    </Card>
  );
}

function formatUnixTime(value: number) {
  return new Date(value * 1000).toLocaleString();
}

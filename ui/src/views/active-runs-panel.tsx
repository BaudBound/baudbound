import { Square } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { DashboardAction } from "@/lib/app-types";
import { stopRun, type ActiveRun } from "@/lib/runner-api";
import { useDesktopTime } from "@/lib/time-format";
import { RunLogPanel } from "@/views/run-log-panel";

export function ActiveRunsPanel({
  busyActions,
  runAction,
  runs,
  scriptNames,
}: {
  busyActions: Set<string>;
  runAction: DashboardAction;
  runs: ActiveRun[];
  scriptNames: Map<string, string>;
}) {
  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between gap-3">
        <CardTitle>Currently running</CardTitle>
        <Badge variant={runs.length > 0 ? "good" : "muted"}>
          {runs.length} active
        </Badge>
      </CardHeader>
      <CardContent className="p-0">
        {runs.length === 0 ? (
          <div className="border-t border-border px-4 py-5 text-sm text-muted-foreground">
            No scripts are currently running.
          </div>
        ) : (
          runs.map((run) => (
            <ActiveRunRow
              busyActions={busyActions}
              key={run.run_id}
              run={run}
              runAction={runAction}
              scriptName={scriptNames.get(run.script_id) ?? run.script_id}
            />
          ))
        )}
      </CardContent>
    </Card>
  );
}

function ActiveRunRow({
  busyActions,
  run,
  runAction,
  scriptName,
}: {
  busyActions: Set<string>;
  run: ActiveRun;
  runAction: DashboardAction;
  scriptName: string;
}) {
  const { formatUnixMilliseconds } = useDesktopTime();
  const stopAction = `stop-run:${run.run_id}`;
  const stopping = run.cancellation_requested || busyActions.has(stopAction);

  return (
    <section className="border-t border-border" aria-label={`Active run ${run.run_id}`}>
      <div className="flex flex-wrap items-start justify-between gap-3 px-4 py-3">
        <div className="grid min-w-0 gap-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="font-medium">{scriptName}</span>
            <Badge variant={stopping ? "medium" : "good"}>
              {stopping ? "stopping" : "running"}
            </Badge>
          </div>
          <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
            <span>Started {formatUnixMilliseconds(run.started_at_unix_ms)}</span>
            <span>Trigger {run.trigger_node_id}</span>
            <span className="font-mono">{run.run_id}</span>
          </div>
        </div>
        <Button
          disabled={stopping}
          onClick={() => runAction(stopAction, () => stopRun(run.run_id))}
          size="sm"
          title={stopping ? "Stop requested" : "Stop this run"}
          variant="destructive"
        >
          <Square />
          {stopping ? "Stopping" : "Stop"}
        </Button>
      </div>
      <div className="border-t border-border bg-background/40 p-4">
        {run.discarded_log_count > 0 ? (
          <div className="mb-3 text-xs text-muted-foreground">
            {run.discarded_log_count} older live log
            {run.discarded_log_count === 1 ? " was" : "s were"} omitted from this
            preview.
          </div>
        ) : null}
        <RunLogPanel
          emptyMessage="Waiting for the first live log entry."
          followOutputControl
          logs={run.logs}
        />
      </div>
    </section>
  );
}

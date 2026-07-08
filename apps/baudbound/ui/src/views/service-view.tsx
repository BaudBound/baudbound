import { Activity, Clock3, Play, RotateCcw, Square } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import type { DashboardAction } from "@/lib/app-types";
import {
  type DashboardPayload,
  reloadBackgroundRunner,
  startBackgroundRunner,
  stopBackgroundRunner,
} from "@/lib/runner-api";
import { ServiceRuntimePanel } from "@/views/service-runtime-panel";

export function ServiceView({
  busyActions,
  dashboard,
  runAction,
}: {
  busyActions: Set<string>;
  dashboard: DashboardPayload;
  runAction: DashboardAction;
}) {
  return (
    <div className="grid gap-4">
      <ServiceControlPanel
        busyActions={busyActions}
        dashboard={dashboard}
        runAction={runAction}
      />

      <ServiceRuntimePanel dashboard={dashboard} />
    </div>
  );
}

function ServiceControlPanel({
  busyActions,
  dashboard,
  runAction,
}: {
  busyActions: Set<string>;
  dashboard: DashboardPayload;
  runAction: DashboardAction;
}) {
  const desktopRunner = dashboard.desktop_background;
  const desktopRunnerRunning =
    desktopRunner.state === "running" || desktopRunner.state === "stopping";

  return (
    <Card>
      <CardContent className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-start">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <div className="text-sm font-medium">Desktop background runner</div>
            <Badge variant={desktopRunnerRunning ? "good" : "muted"}>
              {desktopRunner.state}
            </Badge>
          </div>
          <div className="text-xs text-muted-foreground">
            Runs trigger listeners inside this desktop app. Closing the service loop stops
            schedules, webhooks, serial input, and other listener based triggers.
          </div>
          <div className="mt-3 grid gap-2 text-xs text-muted-foreground sm:grid-cols-2">
            <div className="flex items-center gap-2 rounded-md border border-border bg-background px-3 py-2">
              <Activity className="size-4" />
              <span>{desktopRunner.message}</span>
            </div>
            <div className="flex items-center gap-2 rounded-md border border-border bg-background px-3 py-2">
              <Clock3 className="size-4" />
              <span>
                {desktopRunner.started_at_unix
                  ? `Started ${formatUnix(desktopRunner.started_at_unix)}`
                  : desktopRunner.stopped_at_unix
                    ? `Stopped ${formatUnix(desktopRunner.stopped_at_unix)}`
                    : "No runtime timestamp yet"}
              </span>
            </div>
          </div>
        </div>

        <div className="grid grid-cols-3 gap-2 lg:flex lg:justify-end">
          <Button
            className="w-full lg:w-auto"
            disabled={desktopRunnerRunning || busyActions.has("background-start")}
            onClick={() => runAction("background-start", () => startBackgroundRunner())}
            variant="secondary"
          >
            <Play />
            {busyActions.has("background-start") ? "Working..." : "Start"}
          </Button>
          <Button
            className="w-full lg:w-auto"
            disabled={!desktopRunnerRunning || busyActions.has("background-reload")}
            onClick={() => runAction("background-reload", () => reloadBackgroundRunner())}
            variant="outline"
          >
            <RotateCcw />
            {busyActions.has("background-reload") ? "Working..." : "Reload"}
          </Button>
          <Button
            className="w-full lg:w-auto"
            disabled={!desktopRunnerRunning || busyActions.has("background-stop")}
            onClick={() => runAction("background-stop", () => stopBackgroundRunner())}
            variant="destructive"
          >
            <Square />
            {busyActions.has("background-stop") ? "Working..." : "Stop"}
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}

function formatUnix(value: number) {
  return new Date(value * 1000).toLocaleString();
}

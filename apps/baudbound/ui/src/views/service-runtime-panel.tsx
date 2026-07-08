import { HeartPulse, ListChecks, MonitorCog, RadioTower, Workflow } from "lucide-react";
import type { ReactNode } from "react";

import { Details } from "@/components/details";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { DashboardPayload, ServiceStatusService } from "@/lib/runner-api";
import {
  desktopRuntimeHealth,
  type RuntimeHealth,
} from "@/lib/service-health";

export function ServiceRuntimePanel({ dashboard }: { dashboard: DashboardPayload }) {
  const desktop = desktopRuntimeHealth(dashboard);
  const serviceStatus = dashboard.service_status;
  const serviceHealth = dashboard.service_health;
  const activity = serviceStatus?.activity;

  return (
    <div className="grid gap-4 xl:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]">
      <Card>
        <CardHeader>
          <CardTitle>Desktop runner loop</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-3">
          <RuntimeTile
            detailRows={[
              ["State", dashboard.desktop_background.state],
              [
                "Started",
                dashboard.desktop_background.started_at_unix
                  ? formatUnix(dashboard.desktop_background.started_at_unix)
                  : "not running",
              ],
              [
                "Stopped",
                dashboard.desktop_background.stopped_at_unix
                  ? formatUnix(dashboard.desktop_background.stopped_at_unix)
                  : "not recorded",
              ],
            ]}
            health={desktop}
            icon={<MonitorCog className="size-4 text-muted-foreground" />}
            title="Desktop background"
          />

          <div className="rounded-md border border-border bg-card/60 p-3">
            <div className="flex items-center justify-between gap-3">
              <div className="flex min-w-0 items-center gap-2">
                <HeartPulse className="size-4 text-muted-foreground" />
                <div className="truncate text-sm font-medium">Listener heartbeat</div>
              </div>
              <Badge variant={serviceHealth.ok ? "good" : "medium"}>
                {serviceHealth.health}
              </Badge>
            </div>
            <p className="mt-2 text-xs text-muted-foreground">{serviceHealth.reason}</p>
            <div className="mt-3">
              <Details
                rows={[
                  [
                    "Last heartbeat",
                    serviceStatus?.last_heartbeat_unix
                      ? formatUnix(serviceStatus.last_heartbeat_unix)
                      : "not written",
                  ],
                  [
                    "Age",
                    typeof serviceHealth.heartbeat_age_seconds === "number"
                      ? `${serviceHealth.heartbeat_age_seconds}s`
                      : "unknown",
                  ],
                  [
                    "Stale after",
                    typeof serviceHealth.stale_after_seconds === "number"
                      ? `${serviceHealth.stale_after_seconds}s`
                      : "unknown",
                  ],
                  ["Process", serviceStatus?.pid ? serviceStatus.pid.toString() : "not running"],
                ]}
              />
            </div>
          </div>

          <div className="rounded-md border border-border bg-card/60 p-3">
            <div className="flex items-center justify-between gap-3">
              <div className="flex min-w-0 items-center gap-2">
                <Workflow className="size-4 text-muted-foreground" />
                <div className="truncate text-sm font-medium">Trigger dispatch activity</div>
              </div>
              <Badge variant={activity?.failed_dispatch_count ? "medium" : "muted"}>
                {activity?.total_dispatch_count ?? 0} dispatched
              </Badge>
            </div>
            <div className="mt-3 grid gap-2 sm:grid-cols-2">
              <ServiceFact
                label="Completed or attempted"
                value={(activity?.total_dispatch_count ?? 0).toString()}
              />
              <ServiceFact
                label="Failed"
                value={(activity?.failed_dispatch_count ?? 0).toString()}
              />
            </div>
            {activity?.last_dispatch ? (
              <div className="mt-3 rounded-md border border-border bg-background p-3 text-xs">
                <div className="flex flex-wrap items-center gap-2">
                  <Badge
                    variant={
                      activity.last_dispatch.status === "completed" ? "good" : "destructive"
                    }
                  >
                    {activity.last_dispatch.status}
                  </Badge>
                  <span className="text-muted-foreground">
                    {serviceName(activity.last_dispatch.source)} at{" "}
                    {formatUnix(activity.last_dispatch.completed_at_unix)}
                  </span>
                </div>
                <div className="mt-3">
                  <Details
                    rows={[
                      ["Script", activity.last_dispatch.script_id],
                      ["Trigger", activity.last_dispatch.node_id],
                      ["Run", activity.last_dispatch.run_id ?? "-"],
                      ["Error", activity.last_dispatch.error ?? "-"],
                    ]}
                  />
                </div>
              </div>
            ) : (
              <p className="mt-3 text-xs text-muted-foreground">
                No trigger dispatch has been recorded for this service run yet.
              </p>
            )}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex flex-row items-center justify-between gap-3">
          <div className="flex min-w-0 items-center gap-2">
            <RadioTower className="size-4 text-muted-foreground" />
            <CardTitle>Trigger listener services</CardTitle>
          </div>
          <Badge variant={serviceStatus?.active_service_count ? "good" : "muted"}>
            {serviceStatus?.active_service_count ?? 0} active
          </Badge>
        </CardHeader>
        <CardContent>
          {!serviceStatus ? (
            <div className="rounded-md border border-border bg-background p-3 text-sm text-muted-foreground">
              No listener service status has been written yet.
            </div>
          ) : (
            <div className="grid gap-2">
              {serviceStatus.services.map((service) => (
                <ListenerServiceRow key={service.name} service={service} />
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

function ListenerServiceRow({ service }: { service: ServiceStatusService }) {
  const diagnostics = service.diagnostics;
  const state = diagnostics?.state ?? (service.active ? "active" : service.enabled ? "waiting" : "disabled");
  const badgeVariant =
    state === "active" ? "good" : state === "stopped" || state === "waiting" ? "medium" : "muted";
  return (
    <div className="grid gap-3 rounded-md border border-border bg-background p-3 text-sm md:grid-cols-[minmax(0,1fr)_auto] md:items-center">
      <div className="min-w-0">
        <div className="flex flex-wrap items-center gap-2">
          <ListChecks className="size-4 text-muted-foreground" />
          <div className="font-medium">{serviceName(service.name)}</div>
          <Badge variant={badgeVariant}>{state}</Badge>
        </div>
        <div className="mt-1 break-words text-xs text-muted-foreground">{service.target}</div>
        {diagnostics?.summary ? (
          <div className="mt-1 break-words text-xs text-muted-foreground">
            {diagnostics.summary}
          </div>
        ) : null}
      </div>
      <div className="grid grid-cols-2 gap-2 text-xs text-muted-foreground sm:grid-cols-3 md:min-w-52">
        <ServiceFact label="Enabled" value={service.enabled ? "yes" : "no"} />
        <ServiceFact label="Running" value={diagnostics?.running ? "yes" : "no"} />
        <ServiceFact label="Registrations" value={service.registrations.toString()} />
      </div>
    </div>
  );
}

function ServiceFact({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded border border-border bg-card px-2 py-1">
      <div>{label}</div>
      <div className="font-medium text-foreground">{value}</div>
    </div>
  );
}

function RuntimeTile({
  detailRows,
  health,
  icon,
  title,
}: {
  detailRows: Array<[string, string]>;
  health: RuntimeHealth;
  icon: ReactNode;
  title: string;
}) {
  return (
    <div className="rounded-md border border-border bg-card/60 p-3">
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2">
          {icon}
          <div className="truncate text-sm font-medium">{title}</div>
        </div>
        <Badge variant={runtimeBadgeVariant(health.state)}>{health.label}</Badge>
      </div>
      <p className="mt-2 text-xs text-muted-foreground">{health.detail}</p>
      <div className="mt-3">
        <Details rows={detailRows} />
      </div>
    </div>
  );
}

function runtimeBadgeVariant(state: RuntimeHealth["state"]) {
  if (state === "active") {
    return "good";
  }
  if (state === "stopping") {
    return "medium";
  }
  if (state === "problem") {
    return "destructive";
  }
  return "muted";
}

function formatUnix(value: number) {
  return new Date(value * 1000).toLocaleString();
}

function serviceName(value: string) {
  return value.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
}

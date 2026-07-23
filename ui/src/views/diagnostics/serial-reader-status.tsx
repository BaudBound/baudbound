import { Cable, CircleAlert } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import type {
  DashboardPayload,
  SerialReaderStatus,
  TriggerRegistrationStatus,
} from "@/lib/runner-api";
import { useDesktopTime } from "@/lib/time-format";

export type SerialTriggerRegistration = TriggerRegistrationStatus & {
  scriptId: string;
  scriptName: string;
};

export function SerialReaderStatusPanel({
  dashboard,
  registrations,
}: {
  dashboard: DashboardPayload;
  registrations: SerialTriggerRegistration[];
}) {
  const serviceLive = serialServiceIsLive(dashboard);
  const readers = serviceLive ? serialReaderStatuses(dashboard) : [];

  return (
    <div className="border-t border-border p-3">
      <div className="mb-2 flex items-center gap-2 text-sm font-medium">
        <Cable className="size-4 text-muted-foreground" />
        {serviceLive ? "Live serial readers" : "Serial readers"}
      </div>
      <div className="grid gap-2">
        {registrations.map((registration) => {
          const configuredDevice = dashboard.serial_devices.find(
            (device) => device.device_id === registration.device_id,
          );
          const reader = readers.find(
            (candidate) =>
              candidate.script_id === registration.scriptId &&
              candidate.node_id === registration.node_id,
          );
          return (
            <SerialReaderRow
              configuredDtr={configuredDevice?.dtr_on_open ?? null}
              configuredOpenStabilizationMs={configuredDevice?.open_stabilization_ms ?? null}
              configuredPort={configuredDevice?.port ?? null}
              key={`${registration.scriptId}-${registration.node_id}`}
              reader={reader ?? null}
              registration={registration}
              subscriptionCount={registrations.filter(
                (candidate) => candidate.device_id === registration.device_id,
              ).length}
            />
          );
        })}
      </div>
    </div>
  );
}

function SerialReaderRow({
  configuredDtr,
  configuredOpenStabilizationMs,
  configuredPort,
  reader,
  registration,
  subscriptionCount,
}: {
  configuredDtr: string | null;
  configuredOpenStabilizationMs: number | null;
  configuredPort: string | null;
  reader: SerialReaderStatus | null;
  registration: SerialTriggerRegistration;
  subscriptionCount: number;
}) {
  const { formatUnixSeconds } = useDesktopTime();
  return (
    <div className="grid min-w-0 gap-3 rounded-md border border-border bg-background p-3 text-sm lg:grid-cols-2 2xl:grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)_minmax(0,1fr)_minmax(0,1.4fr)]">
      <div className="min-w-0">
        <div className="flex flex-wrap items-center gap-2">
          <span className="font-medium">{registration.scriptName}</span>
          <Badge
            variant={reader ? readerVariant(reader.state) : configuredPort ? "muted" : "medium"}
          >
            {reader ? readerLabel(reader.state) : configuredPort ? "Not running" : "Missing config"}
          </Badge>
        </div>
        <div className="mt-1 break-all font-mono text-xs text-muted-foreground">
          {registration.node_id}
        </div>
      </div>
      <Fact label="Device ID" value={registration.device_id ?? "Not set"} />
      <div className="grid gap-1.5">
        <Fact label="Configured port" value={configuredPort ?? "Not configured"} />
        <Fact label="Active port" value={reader?.port || "Not connected"} />
        <Fact
          label="Port open behavior"
          value={
            configuredDtr === null || configuredOpenStabilizationMs === null
              ? "Not configured"
              : `${dtrLabel(configuredDtr)}, wait ${configuredOpenStabilizationMs} ms`
          }
        />
        <Fact label="Subscriptions" value={String(subscriptionCount)} />
      </div>
      {reader ? (
        <div className="grid gap-1.5">
          <Fact
            label="Reconnect"
            value={`${reader.auto_reconnect ? "Automatic" : "Off"}, port rebind ${reader.auto_rebind_port ? "on" : "off"}`}
          />
          <Fact
            label="Message framing"
            value={`${readModeLabel(reader.read_mode)}, ${reader.buffered_bytes} bytes buffered`}
          />
          <Fact
            label="Last event"
            value={reader.last_event_unix ? formatUnixSeconds(reader.last_event_unix) : "None"}
          />
          <Fact
            label="Last error"
            value={formatReaderError(reader, formatUnixSeconds)}
            warning={Boolean(reader.last_error || reader.last_framing_error)}
          />
          <Fact
            label="Last port rebind"
            value={formatRebindResult(reader, formatUnixSeconds)}
            warning={reader.last_rebind_result?.startsWith("failed:") ?? false}
          />
        </div>
      ) : (
        <div className="flex min-w-0 items-start gap-2 text-xs text-muted-foreground">
          <CircleAlert className="mt-0.5 size-3.5 shrink-0" />
          <span>
            {configuredPort
              ? "Start the desktop background runner to load this reader."
              : "Add this logical device ID in Config or with the Tools scanner."}
          </span>
        </div>
      )}
    </div>
  );
}

function Fact({
  label,
  value,
  warning = false,
}: {
  label: string;
  value: string;
  warning?: boolean;
}) {
  return (
    <div className="min-w-0">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className={warning ? "mt-0.5 break-words text-baud-amber" : "mt-0.5 break-words"}>
        {value}
      </div>
    </div>
  );
}

function serialReaderStatuses(dashboard: DashboardPayload) {
  const readers =
    dashboard.service_status?.services.find((service) => service.name === "serial_input")?.details
      ?.readers ?? [];
  return readers.filter(isCompleteSerialReaderStatus);
}

export function serialServiceIsLive(
  dashboard: Pick<DashboardPayload, "service_health" | "service_status">,
) {
  return dashboard.service_status?.state === "running" && !dashboard.service_health.stale;
}

function isCompleteSerialReaderStatus(reader: SerialReaderStatus) {
  return (
    typeof reader.auto_reconnect === "boolean" &&
    typeof reader.auto_rebind_port === "boolean" &&
    typeof reader.buffered_bytes === "number" &&
    typeof reader.device_id === "string" &&
    typeof reader.node_id === "string" &&
    typeof reader.port === "string" &&
    typeof reader.read_mode === "string" &&
    typeof reader.script_id === "string" &&
    typeof reader.state === "string"
  );
}

function readerVariant(state: string): "good" | "medium" | "muted" | "red" {
  if (state === "connected" || state === "reading") return "good";
  if (state.endsWith("_failed")) return "red";
  if (state === "stopped") return "muted";
  return "medium";
}

function readerLabel(state: string) {
  return state
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function formatReaderError(reader: SerialReaderStatus, formatUnixSeconds: (value: number) => string) {
  const error = reader.last_framing_error ?? reader.last_error;
  const timestamp = reader.last_framing_error
    ? reader.last_framing_error_unix
    : reader.last_error_unix;
  if (!error) return "None";
  return timestamp ? `${error} (${formatUnixSeconds(timestamp)})` : error;
}

function readModeLabel(mode: string) {
  if (mode === "idle_gap") return "Idle gap";
  if (mode === "line") return "Line endings";
  if (mode === "raw") return "Raw chunks";
  return mode;
}

function dtrLabel(value: string) {
  if (value === "deasserted") return "DTR deasserted";
  if (value === "asserted") return "DTR asserted";
  if (value === "preserve") return "DTR preserved";
  return value;
}

function formatRebindResult(
  reader: SerialReaderStatus,
  formatUnixSeconds: (value: number) => string,
) {
  if (!reader.last_rebind_result) return "None";
  return reader.last_rebind_unix
    ? `${reader.last_rebind_result} (${formatUnixSeconds(reader.last_rebind_unix)})`
    : reader.last_rebind_result;
}

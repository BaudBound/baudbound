import { Cable, CircleAlert } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import type {
  DashboardPayload,
  SerialReaderStatus,
  TriggerRegistrationStatus,
} from "@/lib/runner-api";
import { useDesktopTime } from "@/lib/time-format";

export type SerialTriggerRow = TriggerRegistrationStatus & {
  scriptId: string;
  scriptName: string;
};

export function SerialReaderStatusPanel({
  dashboard,
  registrations,
}: {
  dashboard: DashboardPayload;
  registrations: SerialTriggerRow[];
}) {
  const readers = serialReaderStatuses(dashboard);

  return (
    <div className="border-t border-border p-3">
      <div className="mb-2 flex items-center gap-2 text-sm font-medium">
        <Cable className="size-4 text-muted-foreground" />
        Live serial readers
      </div>
      <div className="grid gap-2">
        {registrations.map((registration) => {
          const reader = readers.find(
            (candidate) =>
              candidate.script_id === registration.scriptId &&
              candidate.node_id === registration.node_id,
          );
          return (
            <SerialReaderRow
              configured={dashboard.serial_devices.some(
                (device) => device.device_id === registration.device_id,
              )}
              key={`${registration.scriptId}-${registration.node_id}`}
              reader={reader ?? null}
              registration={registration}
            />
          );
        })}
      </div>
    </div>
  );
}

function SerialReaderRow({
  configured,
  reader,
  registration,
}: {
  configured: boolean;
  reader: SerialReaderStatus | null;
  registration: SerialTriggerRow;
}) {
  const { formatUnixSeconds } = useDesktopTime();
  return (
    <div className="grid gap-3 rounded-md border border-border bg-background p-3 text-sm md:grid-cols-2 xl:grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)_minmax(0,1fr)_minmax(0,1.4fr)]">
      <div className="min-w-0">
        <div className="flex flex-wrap items-center gap-2">
          <span className="font-medium">{registration.scriptName}</span>
          <Badge variant={reader ? readerVariant(reader.state) : configured ? "muted" : "medium"}>
            {reader ? readerLabel(reader.state) : configured ? "Not running" : "Missing config"}
          </Badge>
        </div>
        <div className="mt-1 break-all font-mono text-xs text-muted-foreground">
          {registration.node_id}
        </div>
      </div>
      <Fact label="Device ID" value={registration.device_id ?? "Not set"} />
      <Fact label="Active port" value={reader?.port || "Not connected"} />
      {reader ? (
        <div className="grid gap-1.5">
          <Fact
            label="Reconnect"
            value={`${reader.auto_reconnect ? "Automatic" : "Off"}, port rebind ${reader.auto_rebind_port ? "on" : "off"}`}
          />
          <Fact
            label="Last event"
            value={reader.last_event_unix ? formatUnixSeconds(reader.last_event_unix) : "None"}
          />
          <Fact
            label="Last error"
            value={formatReaderError(reader, formatUnixSeconds)}
            warning={Boolean(reader.last_error)}
          />
        </div>
      ) : (
        <div className="flex min-w-0 items-start gap-2 text-xs text-muted-foreground">
          <CircleAlert className="mt-0.5 size-3.5 shrink-0" />
          <span>
            {configured
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
  return (
    dashboard.service_status?.services.find((service) => service.name === "serial_input")?.details
      ?.readers ?? []
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
  if (!reader.last_error) return "None";
  return reader.last_error_unix
    ? `${reader.last_error} (${formatUnixSeconds(reader.last_error_unix)})`
    : reader.last_error;
}

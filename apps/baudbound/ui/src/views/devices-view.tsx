import { Cable, Settings, TriangleAlert } from "lucide-react";
import { useMemo } from "react";

import { EmptyState } from "@/components/empty-state";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type {
  DashboardPayload,
  SerialDeviceStatus,
  TriggerRegistrationStatus,
} from "@/lib/runner-api";

type DeviceReference = {
  scriptId: string;
  scriptName: string;
  trigger: TriggerRegistrationStatus;
};

type DeviceRow = {
  configured: SerialDeviceStatus | null;
  deviceId: string;
  references: DeviceReference[];
};

export function DevicesView({ dashboard }: { dashboard: DashboardPayload }) {
  const serialDevices = dashboard.serial_devices ?? [];
  const rows = useMemo(() => deviceRows(dashboard), [dashboard]);
  const missingConfigCount = rows.filter((row) => !row.configured).length;
  const referencedDeviceCount = rows.filter((row) => row.references.length > 0).length;

  return (
    <div className="grid gap-4">
      <div className="grid grid-cols-4 gap-3 max-lg:grid-cols-2 max-sm:grid-cols-1">
        <DeviceMetric label="Configured" tone="good" value={serialDevices.length} />
        <DeviceMetric label="Referenced" value={referencedDeviceCount} />
        <DeviceMetric
          label="Serial triggers"
          value={rows.reduce((total, row) => total + row.references.length, 0)}
        />
        <DeviceMetric
          label="Missing config"
          tone={missingConfigCount > 0 ? "medium" : "muted"}
          value={missingConfigCount}
        />
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Device configuration model</CardTitle>
        </CardHeader>
        <CardContent className="grid gap-3 text-sm text-muted-foreground">
          <div className="flex gap-2">
            <Settings className="mt-0.5 size-4 shrink-0" />
            <span>
              Script packages reference logical device IDs. Ports, baud rate, reconnect behavior,
              and USB identity validation are configured locally in the runner Config tab.
            </span>
          </div>
          <div className="flex gap-2">
            <Cable className="mt-0.5 size-4 shrink-0" />
            <span>
              This view now validates script references against the configured runner devices.
              Live connection telemetry can be added once the serial listener exposes it.
            </span>
          </div>
        </CardContent>
      </Card>

      {rows.length === 0 ? (
        <EmptyState>No serial devices are configured or referenced by installed scripts.</EmptyState>
      ) : (
        <div className="grid gap-4">
          {rows.map((row) => (
            <DeviceCard key={row.deviceId} row={row} />
          ))}
        </div>
      )}
    </div>
  );
}

function DeviceCard({ row }: { row: DeviceRow }) {
  const configured = row.configured;
  return (
    <Card>
      <CardHeader className="flex flex-row items-start justify-between gap-3">
        <div className="min-w-0">
          <CardTitle>{row.deviceId}</CardTitle>
          <div className="mt-1 text-xs text-muted-foreground">
            {configured ? configured.port || "Port not set" : "No matching runner config"}
          </div>
        </div>
        <Badge variant={configured ? "good" : "medium"}>
          {configured ? "Configured" : "Missing config"}
        </Badge>
      </CardHeader>
      <CardContent className="grid gap-4">
        {configured ? (
          <div className="grid gap-2 rounded-md border border-border bg-background p-3 text-sm sm:grid-cols-2 xl:grid-cols-4">
            <DeviceFact label="Baud" value={configured.baud_rate.toString()} />
            <DeviceFact label="Read mode" value={configured.read_mode} />
            <DeviceFact label="Reconnect" value={configured.auto_reconnect ? "on" : "off"} />
            <DeviceFact
              label="USB validation"
              value={configured.validate_usb_identity ? "on" : "off"}
            />
            <DeviceFact label="Data bits" value={configured.data_bits.toString()} />
            <DeviceFact label="Parity" value={configured.parity} />
            <DeviceFact label="Stop bits" value={configured.stop_bits} />
            <DeviceFact label="Flow" value={configured.flow_control} />
            <DeviceFact label="Vendor ID" value={configured.vendor_id ?? "-"} />
            <DeviceFact label="Product ID" value={configured.product_id ?? "-"} />
          </div>
        ) : (
          <div className="flex gap-2 rounded-md border border-baud-amber/30 bg-baud-amber/10 p-3 text-sm text-baud-amber">
            <TriangleAlert className="mt-0.5 size-4 shrink-0" />
            <span>
              One or more scripts reference this device ID, but the runner Config tab does not
              define matching serial settings.
            </span>
          </div>
        )}

        {row.references.length === 0 ? (
          <div className="rounded-md border border-border bg-background p-3 text-sm text-muted-foreground">
            No installed script currently references this device.
          </div>
        ) : (
          <div>
            <div className="mb-2 text-sm font-medium">Script references</div>
            <div className="grid gap-2">
              {row.references.map((reference) => (
                <div
                  className="grid gap-2 rounded-md border border-border bg-background p-3 text-sm md:grid-cols-[minmax(0,1fr)_160px_160px]"
                  key={`${reference.scriptId}-${reference.trigger.node_id}`}
                >
                  <div className="min-w-0">
                    <div className="truncate font-medium">{reference.scriptName}</div>
                    <div className="font-mono text-xs text-muted-foreground">
                      {reference.scriptId}
                    </div>
                  </div>
                  <div>
                    <div className="text-xs text-muted-foreground">Node</div>
                    <div className="font-mono text-xs">{reference.trigger.node_id}</div>
                  </div>
                  <div>
                    <div className="text-xs text-muted-foreground">Runner</div>
                    <Badge variant="muted">{reference.trigger.runner_type}</Badge>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function DeviceFact({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="mt-0.5 break-words font-medium">{value}</div>
    </div>
  );
}

function DeviceMetric({
  label,
  tone = "medium",
  value,
}: {
  label: string;
  tone?: "good" | "medium" | "muted";
  value: number;
}) {
  return (
    <Card>
      <CardContent className="flex items-center justify-between gap-3">
        <div>
          <div className="text-sm text-muted-foreground">{label}</div>
          <div className="mt-1 text-2xl font-semibold">{value}</div>
        </div>
        <Badge variant={tone}>{label}</Badge>
      </CardContent>
    </Card>
  );
}

function deviceRows(dashboard: DashboardPayload): DeviceRow[] {
  const serialDevices = dashboard.serial_devices ?? [];
  const configured = new Map(
    serialDevices.map((device) => [device.device_id, device]),
  );
  const references = serialReferences(dashboard);
  const deviceIds = new Set([
    ...configured.keys(),
    ...references.map((reference) => reference.trigger.device_id).filter(isString),
  ]);

  return Array.from(deviceIds)
    .sort((a, b) => a.localeCompare(b))
    .map((deviceId) => ({
      configured: configured.get(deviceId) ?? null,
      deviceId,
      references: references.filter((reference) => reference.trigger.device_id === deviceId),
    }));
}

function serialReferences(dashboard: DashboardPayload): DeviceReference[] {
  return (dashboard.runner.scripts ?? []).flatMap((script) =>
    (script.triggers ?? [])
      .filter((trigger) => trigger.action_type === "trigger.serial_input")
      .filter((trigger) => isString(trigger.device_id))
      .map((trigger) => ({
        scriptId: script.installed.id,
        scriptName: script.installed.name,
        trigger,
      })),
  );
}

function isString(value: string | null): value is string {
  return typeof value === "string" && value.length > 0;
}

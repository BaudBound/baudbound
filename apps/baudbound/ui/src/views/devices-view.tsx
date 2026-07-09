import { Cable, Plus, RefreshCcw, Settings, TriangleAlert } from "lucide-react";
import { useMemo, useState } from "react";
import { toast } from "sonner";

import { EmptyState } from "@/components/empty-state";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import type { DashboardAction } from "@/lib/app-types";
import type {
  DashboardPayload,
  SerialDeviceStatus,
  SerialDeviceSettings,
  SerialPortScanResult,
  SerialReaderStatus,
  TriggerRegistrationStatus,
} from "@/lib/runner-api";
import {
  readRunnerConfig,
  saveRunnerConfigModel,
  scanSerialPorts,
} from "@/lib/runner-api";

type DeviceReference = {
  scriptId: string;
  scriptName: string;
  trigger: TriggerRegistrationStatus;
};

type DeviceRow = {
  configured: SerialDeviceStatus | null;
  deviceId: string;
  readers: SerialReaderStatus[];
  references: DeviceReference[];
};

export function DevicesView({
  busyActions,
  dashboard,
  runAction,
}: {
  busyActions: Set<string>;
  dashboard: DashboardPayload;
  runAction: DashboardAction;
}) {
  const serialDevices = dashboard.serial_devices ?? [];
  const serialReaders = useMemo(() => serialReaderStatuses(dashboard), [dashboard]);
  const rows = useMemo(() => deviceRows(dashboard, serialReaders), [dashboard, serialReaders]);
  const missingConfigCount = rows.filter((row) => !row.configured).length;
  const referencedDeviceCount = rows.filter((row) => row.references.length > 0).length;
  const activeReaderCount = serialReaders.filter((reader) => isConnectedState(reader.state)).length;
  const [isScanning, setIsScanning] = useState(false);
  const [scanResults, setScanResults] = useState<SerialPortScanResult[]>([]);
  const [selectedPort, setSelectedPort] = useState<SerialPortScanResult | null>(null);

  async function scanPorts() {
    setIsScanning(true);
    try {
      const ports = await scanSerialPorts();
      setScanResults(ports);
      toast.success(`Found ${ports.length} serial port${ports.length === 1 ? "" : "s"}.`);
    } catch (error) {
      toast.error(String(error));
    } finally {
      setIsScanning(false);
    }
  }

  return (
    <div className="grid gap-4">
      <div className="grid grid-cols-5 gap-3 max-xl:grid-cols-3 max-lg:grid-cols-2 max-sm:grid-cols-1">
        <DeviceMetric label="Configured" tone="good" value={serialDevices.length} />
        <DeviceMetric label="Referenced" value={referencedDeviceCount} />
        <DeviceMetric
          label="Serial triggers"
          value={rows.reduce((total, row) => total + row.references.length, 0)}
        />
        <DeviceMetric label="Connected readers" tone="good" value={activeReaderCount} />
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
              This view validates script references and shows live listener state when the
              background runner is active.
            </span>
          </div>
        </CardContent>
      </Card>

      <SerialScanner
        busyActions={busyActions}
        configuredDeviceIds={new Set(serialDevices.map((device) => device.device_id))}
        isScanning={isScanning}
        onAddPort={setSelectedPort}
        onScan={scanPorts}
        ports={scanResults}
        runAction={runAction}
        selectedPort={selectedPort}
        setSelectedPort={setSelectedPort}
      />

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

function SerialScanner({
  busyActions,
  configuredDeviceIds,
  isScanning,
  onAddPort,
  onScan,
  ports,
  runAction,
  selectedPort,
  setSelectedPort,
}: {
  busyActions: Set<string>;
  configuredDeviceIds: Set<string>;
  isScanning: boolean;
  onAddPort: (port: SerialPortScanResult) => void;
  onScan: () => void;
  ports: SerialPortScanResult[];
  runAction: DashboardAction;
  selectedPort: SerialPortScanResult | null;
  setSelectedPort: (port: SerialPortScanResult | null) => void;
}) {
  return (
    <Card>
      <CardHeader className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <CardTitle>Serial device scanner</CardTitle>
          <div className="mt-1 text-sm text-muted-foreground">
            Scan connected serial ports and add one to the runner config as a logical device id.
          </div>
        </div>
        <Button disabled={isScanning} onClick={onScan} variant="secondary">
          <RefreshCcw className={isScanning ? "animate-spin" : ""} />
          Scan
        </Button>
      </CardHeader>
      <CardContent>
        {ports.length === 0 ? (
          <div className="rounded-md border border-border bg-background px-3 py-3 text-sm text-muted-foreground">
            No scan results yet. Run a scan to list connected serial ports.
          </div>
        ) : (
          <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
            {ports.map((port) => (
              <ScannedPortCard
                key={`${port.port}-${port.port_type}-${port.vendor_id ?? ""}-${port.product_id ?? ""}`}
                onAdd={() => onAddPort(port)}
                port={port}
              />
            ))}
          </div>
        )}
      </CardContent>
      <AddScannedPortDialog
        busyActions={busyActions}
        configuredDeviceIds={configuredDeviceIds}
        onOpenChange={(open) => {
          if (!open) setSelectedPort(null);
        }}
        port={selectedPort}
        runAction={runAction}
      />
    </Card>
  );
}

function ScannedPortCard({
  onAdd,
  port,
}: {
  onAdd: () => void;
  port: SerialPortScanResult;
}) {
  return (
    <div className="grid gap-3 rounded-md border border-border bg-background p-3 text-sm">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="truncate font-medium">{port.port}</div>
          <div className="mt-1">
            <Badge variant={port.port_type === "usb" ? "good" : "muted"}>
              {serialPortTypeLabel(port.port_type)}
            </Badge>
          </div>
        </div>
        <Button onClick={onAdd} size="sm" variant="secondary">
          <Plus />
          Add
        </Button>
      </div>
      <div className="grid gap-2 sm:grid-cols-2">
        <DeviceFact label="Vendor ID" value={port.vendor_id ?? "-"} />
        <DeviceFact label="Product ID" value={port.product_id ?? "-"} />
        <DeviceFact label="Serial number" value={port.serial_number ?? "-"} />
        <DeviceFact label="Manufacturer" value={port.manufacturer ?? "-"} />
        <DeviceFact label="Product" value={port.product ?? "-"} />
      </div>
    </div>
  );
}

function AddScannedPortDialog({
  busyActions,
  configuredDeviceIds,
  onOpenChange,
  port,
  runAction,
}: {
  busyActions: Set<string>;
  configuredDeviceIds: Set<string>;
  onOpenChange: (open: boolean) => void;
  port: SerialPortScanResult | null;
  runAction: DashboardAction;
}) {
  const [deviceId, setDeviceId] = useState("");
  const normalizedDeviceId = normalizeDeviceId(deviceId);
  const isDuplicate = normalizedDeviceId.length > 0 && configuredDeviceIds.has(normalizedDeviceId);
  const actionId = `serial-device-add-${port?.port ?? "none"}-${normalizedDeviceId}`;
  const isBusy = busyActions.has(actionId);

  async function addDevice() {
    if (!port || normalizedDeviceId.length === 0 || isDuplicate) return;
    const added = await runAction(actionId, async () => {
      const payload = await readRunnerConfig();
      if (payload.config.serial.devices[normalizedDeviceId]) {
        throw new Error(`Serial device id "${normalizedDeviceId}" already exists.`);
      }
      return saveRunnerConfigModel(
        {
          ...payload.config,
          serial: {
            devices: {
              ...payload.config.serial.devices,
              [normalizedDeviceId]: serialDeviceFromPort(port),
            },
          },
        },
        true,
      );
    });
    if (added) {
      setDeviceId("");
      onOpenChange(false);
    }
  }

  return (
    <Dialog
      onOpenChange={(open) => {
        if (!open) setDeviceId("");
        onOpenChange(open);
      }}
      open={Boolean(port)}
    >
      <DialogContent className="w-[min(calc(100vw-2rem),520px)]">
        <DialogHeader>
          <DialogTitle>Add serial device</DialogTitle>
          <DialogDescription>
            Choose the logical device id that Serial Input Trigger nodes will reference.
          </DialogDescription>
        </DialogHeader>
        {port ? (
          <div className="grid gap-4">
            <div className="rounded-md border border-border bg-background p-3 text-sm">
              <DeviceFact label="Port" value={port.port} />
              <div className="mt-3 grid gap-2 sm:grid-cols-2">
                <DeviceFact label="Vendor ID" value={port.vendor_id ?? "-"} />
                <DeviceFact label="Product ID" value={port.product_id ?? "-"} />
                <DeviceFact label="Serial number" value={port.serial_number ?? "-"} />
                <DeviceFact label="Manufacturer" value={port.manufacturer ?? "-"} />
              </div>
            </div>
            <label className="grid gap-1.5 text-sm">
              <span className="text-xs text-muted-foreground">Device ID</span>
              <Input
                autoFocus
                onChange={(event) => setDeviceId(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") void addDevice();
                }}
                placeholder="main_controller"
                value={deviceId}
              />
            </label>
            {isDuplicate ? (
              <div className="rounded-md border border-baud-amber/30 bg-baud-amber/10 px-3 py-2 text-xs text-baud-amber">
                That device id already exists in the runner config.
              </div>
            ) : null}
          </div>
        ) : null}
        <DialogFooter>
          <Button onClick={() => onOpenChange(false)} variant="outline">
            Cancel
          </Button>
          <Button
            disabled={!port || normalizedDeviceId.length === 0 || isDuplicate || isBusy}
            onClick={() => void addDevice()}
          >
            <Plus />
            Add device
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
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
              label="Port rebind"
              value={configured.auto_rebind_port ? "on" : "off"}
            />
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
            <DeviceFact label="Serial number" value={configured.serial_number ?? "-"} />
            <DeviceFact label="Manufacturer" value={configured.manufacturer ?? "-"} />
            <DeviceFact label="Product" value={configured.product ?? "-"} />
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

        {row.readers.length > 0 ? (
          <div>
            <div className="mb-2 text-sm font-medium">Live listener state</div>
            <div className="grid gap-2">
              {row.readers.map((reader) => (
                <SerialReaderCard
                  key={`${reader.script_id}-${reader.node_id}`}
                  reader={reader}
                />
              ))}
            </div>
          </div>
        ) : (
          <div className="rounded-md border border-border bg-background p-3 text-sm text-muted-foreground">
            No active serial reader is reporting for this device.
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

function SerialReaderCard({ reader }: { reader: SerialReaderStatus }) {
  return (
    <div className="grid gap-3 rounded-md border border-border bg-background p-3 text-sm lg:grid-cols-[minmax(0,1fr)_140px_160px_minmax(0,1.25fr)]">
      <div className="min-w-0">
        <div className="mb-1 flex flex-wrap items-center gap-2">
          <Badge variant={serialReaderVariant(reader.state)}>
            {serialReaderLabel(reader.state)}
          </Badge>
          <span className="text-xs text-muted-foreground">
            {reader.auto_reconnect ? "Auto reconnect on" : "Auto reconnect off"}
            {reader.auto_rebind_port ? " · port rebind on" : ""}
          </span>
        </div>
        <div className="truncate font-medium">{reader.port || "Port not set"}</div>
        <div className="font-mono text-xs text-muted-foreground">{reader.script_id}</div>
      </div>
      <DeviceFact label="Node" value={reader.node_id} />
      <DeviceFact
        label="Last event"
        value={reader.last_event_unix ? formatUnix(reader.last_event_unix) : "none"}
      />
      <DeviceFact
        label="Last error"
        value={
          reader.last_error
            ? `${reader.last_error}${reader.last_error_unix ? ` (${formatUnix(reader.last_error_unix)})` : ""}`
            : "none"
        }
      />
    </div>
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

function deviceRows(
  dashboard: DashboardPayload,
  serialReaders: SerialReaderStatus[],
): DeviceRow[] {
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
      readers: serialReaders.filter((reader) => reader.device_id === deviceId),
      references: references.filter((reference) => reference.trigger.device_id === deviceId),
    }));
}

function serialReaderStatuses(dashboard: DashboardPayload): SerialReaderStatus[] {
  return (
    dashboard.service_status?.services.find((service) => service.name === "serial_input")
      ?.details?.readers ?? []
  );
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

function isConnectedState(state: string) {
  return state === "connected" || state === "reading";
}

function serialReaderVariant(state: string): "good" | "medium" | "muted" | "red" {
  if (isConnectedState(state)) {
    return "good";
  }
  if (state.endsWith("_failed")) {
    return "red";
  }
  if (state === "stopped") {
    return "muted";
  }
  return "medium";
}

function serialReaderLabel(state: string) {
  return state
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function formatUnix(value: number) {
  return new Date(value * 1000).toLocaleString();
}

function serialDeviceFromPort(port: SerialPortScanResult): SerialDeviceSettings {
  const hasUsbIdentity = Boolean(port.vendor_id && port.product_id);
  return {
    auto_reconnect: true,
    auto_rebind_port: false,
    baud_rate: 115_200,
    data_bits: 8,
    flow_control: "none",
    manufacturer: port.manufacturer,
    parity: "none",
    port: port.port,
    product: port.product,
    product_id: port.product_id,
    read_mode: "line",
    serial_number: port.serial_number,
    stop_bits: "1",
    validate_usb_identity: hasUsbIdentity,
    vendor_id: port.vendor_id,
  };
}

function normalizeDeviceId(value: string) {
  return value.trim().replaceAll(/\s+/g, "_");
}

function serialPortTypeLabel(value: string) {
  if (value === "usb") return "USB";
  if (value === "bluetooth") return "Bluetooth";
  if (value === "pci") return "PCI";
  return "Unknown";
}

import { Plus, RefreshCcw } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { DashboardAction } from "@/lib/app-types";
import type { SerialPortScanResult } from "@/lib/runner-api";
import { scanSerialPorts } from "@/lib/runner-api";
import { AddSerialDeviceDialog } from "@/views/tools/add-serial-device-dialog";
import { SerialDeviceFact } from "@/views/tools/serial-device-fact";
import { serialPortTypeLabel } from "@/views/tools/serial-device-model";

export function SerialScanner({
  busyActions,
  configuredDeviceIds,
  runAction,
}: {
  busyActions: Set<string>;
  configuredDeviceIds: Set<string>;
  runAction: DashboardAction;
}) {
  const [isScanning, setIsScanning] = useState(false);
  const [ports, setPorts] = useState<SerialPortScanResult[]>([]);
  const [selectedPort, setSelectedPort] = useState<SerialPortScanResult | null>(null);

  async function scanPorts() {
    setIsScanning(true);
    try {
      const results = await scanSerialPorts();
      setPorts(results);
      toast.success(`Found ${results.length} serial port${results.length === 1 ? "" : "s"}.`);
    } catch (error) {
      toast.error(String(error));
    } finally {
      setIsScanning(false);
    }
  }

  return (
    <Card>
      <CardHeader className="flex flex-wrap items-center justify-between gap-3">
        <div className="min-w-0">
          <CardTitle>Serial device scanner</CardTitle>
          <div className="mt-1 text-sm text-muted-foreground">
            Scan connected serial ports and add one to the runner config as a logical device ID.
          </div>
        </div>
        <Button disabled={isScanning} onClick={() => void scanPorts()} variant="secondary">
          <RefreshCcw className={isScanning ? "animate-spin" : ""} />
          {isScanning ? "Scanning..." : "Scan"}
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
              <ScannedPort key={portKey(port)} onAdd={() => setSelectedPort(port)} port={port} />
            ))}
          </div>
        )}
      </CardContent>
      <AddSerialDeviceDialog
        busyActions={busyActions}
        configuredDeviceIds={configuredDeviceIds}
        onClose={() => setSelectedPort(null)}
        port={selectedPort}
        runAction={runAction}
      />
    </Card>
  );
}

function ScannedPort({
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
          <div className="break-words font-medium">{port.port}</div>
          <Badge className="mt-1" variant={port.port_type === "usb" ? "good" : "muted"}>
            {serialPortTypeLabel(port.port_type)}
          </Badge>
        </div>
        <Button onClick={onAdd} size="sm" variant="secondary">
          <Plus />
          Add
        </Button>
      </div>
      <div className="grid gap-2 sm:grid-cols-2">
        <SerialDeviceFact label="Vendor ID" value={port.vendor_id ?? "-"} />
        <SerialDeviceFact label="Product ID" value={port.product_id ?? "-"} />
        <SerialDeviceFact label="Serial number" value={port.serial_number ?? "-"} />
        <SerialDeviceFact label="Manufacturer" value={port.manufacturer ?? "-"} />
        <SerialDeviceFact label="Product" value={port.product ?? "-"} />
      </div>
    </div>
  );
}

function portKey(port: SerialPortScanResult) {
  return [port.port, port.port_type, port.vendor_id, port.product_id, port.serial_number].join("|");
}

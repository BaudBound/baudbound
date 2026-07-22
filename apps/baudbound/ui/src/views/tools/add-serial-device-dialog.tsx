import { Plus } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
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
import { SERIAL_DEVICE_ID_MAX_LENGTH } from "@/lib/input-limits";
import type { SerialPortScanResult } from "@/lib/runner-api";
import { readRunnerConfig, saveRunnerConfigModel } from "@/lib/runner-api";
import {
  normalizeSerialDeviceId,
  serialDeviceSettingsFromPort,
} from "@/views/tools/serial-device-model";
import { SerialDeviceFact } from "@/views/tools/serial-device-fact";

export function AddSerialDeviceDialog({
  busyActions,
  configuredDeviceIds,
  onClose,
  port,
  runAction,
}: {
  busyActions: Set<string>;
  configuredDeviceIds: Set<string>;
  onClose: () => void;
  port: SerialPortScanResult | null;
  runAction: DashboardAction;
}) {
  const [deviceId, setDeviceId] = useState("");
  const normalizedDeviceId = normalizeSerialDeviceId(deviceId);
  const duplicate = normalizedDeviceId.length > 0 && configuredDeviceIds.has(normalizedDeviceId);
  const actionId = `serial-device-add-${port?.port ?? "none"}-${normalizedDeviceId}`;

  function close() {
    setDeviceId("");
    onClose();
  }

  async function addDevice() {
    if (!port || normalizedDeviceId.length === 0 || duplicate) return;
    const added = await runAction(actionId, async () => {
      const payload = await readRunnerConfig();
      if (payload.config.serial.devices[normalizedDeviceId]) {
        throw new Error(`Serial device ID "${normalizedDeviceId}" already exists.`);
      }
      return saveRunnerConfigModel(
        {
          ...payload.config,
          serial: {
            devices: {
              ...payload.config.serial.devices,
              [normalizedDeviceId]: serialDeviceSettingsFromPort(port),
            },
          },
        },
        true,
      );
    });
    if (added) close();
  }

  return (
    <Dialog onOpenChange={(open) => !open && close()} open={Boolean(port)}>
      <DialogContent className="w-[min(calc(100vw-2rem),520px)]">
        <DialogHeader>
          <DialogTitle>Add serial device</DialogTitle>
          <DialogDescription>
            Choose the logical device ID that Serial Input nodes will reference.
          </DialogDescription>
        </DialogHeader>
        {port ? (
          <div className="grid gap-4">
            <div className="rounded-md border border-border bg-background p-3 text-sm">
              <SerialDeviceFact label="Port" value={port.port} />
              <div className="mt-3 grid gap-2 sm:grid-cols-2">
                <SerialDeviceFact label="Vendor ID" value={port.vendor_id ?? "-"} />
                <SerialDeviceFact label="Product ID" value={port.product_id ?? "-"} />
                <SerialDeviceFact label="Serial number" value={port.serial_number ?? "-"} />
                <SerialDeviceFact label="Manufacturer" value={port.manufacturer ?? "-"} />
              </div>
            </div>
            <label className="grid gap-1.5 text-sm">
              <span className="text-xs text-muted-foreground">Device ID</span>
              <Input
                autoFocus
                maxLength={SERIAL_DEVICE_ID_MAX_LENGTH}
                onChange={(event) => setDeviceId(event.target.value)}
                onKeyDown={(event) => event.key === "Enter" && void addDevice()}
                placeholder="main_controller"
                value={deviceId}
              />
            </label>
            {duplicate ? (
              <div className="rounded-md border border-baud-amber/30 bg-baud-amber/10 px-3 py-2 text-xs text-baud-amber">
                That device ID already exists in the runner config.
              </div>
            ) : null}
          </div>
        ) : null}
        <DialogFooter>
          <Button onClick={close} variant="outline">
            Cancel
          </Button>
          <Button
            disabled={
              !port ||
              normalizedDeviceId.length === 0 ||
              duplicate ||
              busyActions.has(actionId)
            }
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

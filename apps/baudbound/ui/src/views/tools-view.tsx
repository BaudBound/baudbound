import type { DashboardAction } from "@/lib/app-types";
import type { DashboardPayload } from "@/lib/runner-api";
import { MonitorDiscovery } from "@/views/tools/monitor-discovery";
import { SerialScanner } from "@/views/tools/serial-scanner";

export function ToolsView({
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
      <MonitorDiscovery />
      <SerialScanner
        busyActions={busyActions}
        configuredDeviceIds={new Set(dashboard.serial_devices.map((device) => device.device_id))}
        runAction={runAction}
      />
    </div>
  );
}

import type { TriggerMonitorController } from "@/hooks/use-trigger-monitor";
import type { DashboardAction } from "@/lib/app-types";
import type { DashboardPayload } from "@/lib/runner-api";
import { MonitorDiscovery } from "@/views/tools/monitor-discovery";
import { SerialScanner } from "@/views/tools/serial-scanner";
import { TriggerMonitorPanel } from "@/views/tools/trigger-monitor-panel";

export function ToolsView({
  busyActions,
  dashboard,
  runAction,
  triggerMonitor,
}: {
  busyActions: Set<string>;
  dashboard: DashboardPayload;
  runAction: DashboardAction;
  triggerMonitor: TriggerMonitorController;
}) {
  return (
    <div className="grid gap-4">
      <TriggerMonitorPanel controller={triggerMonitor} dashboard={dashboard} />
      {dashboard.desktop_platform === "windows" ? <MonitorDiscovery /> : null}
      <SerialScanner
        busyActions={busyActions}
        configuredDeviceIds={new Set(dashboard.serial_devices.map((device) => device.device_id))}
        runAction={runAction}
      />
    </div>
  );
}

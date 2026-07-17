import { listen } from "@tauri-apps/api/event";
import { Crosshair, Monitor, RefreshCcw } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type {
  CoordinatePickerEvent,
  CoordinatePickerResult,
  MonitorDiscoveryResult,
  MonitorInfo,
} from "@/lib/runner-api";
import { discoverMonitors, startCoordinatePicker } from "@/lib/runner-api";
import { CoordinatePickerResultView } from "@/views/tools/coordinate-picker-result";
import {
  formatMonitorAxisRange,
  formatMonitorScale,
  formatMonitorSize,
} from "@/views/tools/monitor-model";

const coordinatePickerEvent = "coordinate-picker-finished";

export function MonitorDiscovery() {
  const [isScanning, setIsScanning] = useState(false);
  const [isPicking, setIsPicking] = useState(false);
  const [pickerListenerReady, setPickerListenerReady] = useState(false);
  const [pickerResult, setPickerResult] = useState<CoordinatePickerResult | null>(null);
  const [result, setResult] = useState<MonitorDiscoveryResult | null>(null);

  useEffect(() => {
    let disposed = false;
    let removeListener: (() => void) | undefined;

    void listen<CoordinatePickerEvent>(coordinatePickerEvent, (event) => {
      setIsPicking(false);
      if (event.payload.status === "selected") {
        setPickerResult(event.payload.result);
        toast.success("Screen coordinate selected.");
      } else if (event.payload.status === "cancelled") {
        toast.info("Coordinate selection cancelled.");
      } else {
        toast.error(event.payload.message);
      }
    })
      .then((unlisten) => {
        if (disposed) {
          unlisten();
          return;
        }
        removeListener = unlisten;
        setPickerListenerReady(true);
      })
      .catch((error) => toast.error(`Could not initialize the coordinate picker: ${String(error)}`));

    return () => {
      disposed = true;
      removeListener?.();
    };
  }, []);

  async function scanMonitors() {
    setIsScanning(true);
    try {
      const discovery = await discoverMonitors();
      setResult(discovery);
      if (discovery.supported) {
        toast.success(
          `Found ${discovery.monitors.length} monitor${discovery.monitors.length === 1 ? "" : "s"}.`,
        );
      }
    } catch (error) {
      toast.error(String(error));
    } finally {
      setIsScanning(false);
    }
  }

  async function pickCoordinate() {
    setIsPicking(true);
    try {
      await startCoordinatePicker();
    } catch (error) {
      setIsPicking(false);
      toast.error(String(error));
    }
  }

  return (
    <Card>
      <CardHeader className="flex flex-wrap items-center justify-between gap-3">
        <div className="min-w-0">
          <CardTitle>Screen coordinates</CardTitle>
          <div className="mt-1 text-sm text-muted-foreground">Windows virtual desktop layout.</div>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button
            disabled={!pickerListenerReady || isPicking}
            onClick={() => void pickCoordinate()}
          >
            <Crosshair />
            {isPicking ? "Picker open" : "Pick coordinate"}
          </Button>
          <Button disabled={isScanning} onClick={() => void scanMonitors()} variant="secondary">
            <RefreshCcw className={isScanning ? "animate-spin" : ""} />
            {isScanning ? "Detecting..." : result ? "Refresh" : "Detect monitors"}
          </Button>
        </div>
      </CardHeader>
      <CardContent className="grid gap-3">
        {result?.supported === false ? (
          <div className="rounded-md border border-border bg-background px-3 py-3 text-sm text-muted-foreground">
            {result.unavailable_reason ?? "Monitor discovery is unavailable on this platform."}
          </div>
        ) : result?.virtual_bounds ? (
          <>
            <VirtualDesktopSummary result={result} />
            <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
              {result.monitors.map((monitor) => (
                <MonitorSummary key={monitor.id} monitor={monitor} />
              ))}
            </div>
          </>
        ) : (
          <div className="rounded-md border border-border bg-background px-3 py-3 text-sm text-muted-foreground">
            No monitor scan yet.
          </div>
        )}
        {pickerResult ? <CoordinatePickerResultView result={pickerResult} /> : null}
      </CardContent>
    </Card>
  );
}

function VirtualDesktopSummary({ result }: { result: MonitorDiscoveryResult }) {
  const bounds = result.virtual_bounds;
  if (!bounds) {
    return null;
  }

  return (
    <div className="grid gap-2 rounded-md border border-border bg-background p-3 text-sm sm:grid-cols-3">
      <MonitorFact label="Virtual size" value={formatMonitorSize(bounds)} />
      <MonitorFact label="X coordinates" value={formatMonitorAxisRange(bounds, "x")} />
      <MonitorFact label="Y coordinates" value={formatMonitorAxisRange(bounds, "y")} />
    </div>
  );
}

function MonitorSummary({ monitor }: { monitor: MonitorInfo }) {
  return (
    <div className="grid gap-3 rounded-md border border-border bg-background p-3 text-sm">
      <div className="flex min-w-0 items-start gap-2">
        <Monitor className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
        <div className="min-w-0 flex-1">
          <div className="break-all font-medium">{monitor.device_name}</div>
          {monitor.is_primary ? (
            <Badge className="mt-1" variant="good">
              Primary
            </Badge>
          ) : null}
        </div>
      </div>
      <div className="grid gap-2 sm:grid-cols-2">
        <MonitorFact label="Size" value={formatMonitorSize(monitor.bounds)} />
        <MonitorFact label="X coordinates" value={formatMonitorAxisRange(monitor.bounds, "x")} />
        <MonitorFact label="Y coordinates" value={formatMonitorAxisRange(monitor.bounds, "y")} />
        <MonitorFact label="Scale" value={formatMonitorScale(monitor)} />
      </div>
    </div>
  );
}

function MonitorFact({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="mt-0.5 break-words font-mono text-xs text-foreground">{value}</div>
    </div>
  );
}

import type { MonitorBounds, MonitorInfo } from "@/lib/runner-api";

export function formatMonitorSize(bounds: MonitorBounds) {
  return `${bounds.width} x ${bounds.height}`;
}

export function formatMonitorAxisRange(bounds: MonitorBounds, axis: "x" | "y") {
  return axis === "x"
    ? `${bounds.left} to ${bounds.right - 1}`
    : `${bounds.top} to ${bounds.bottom - 1}`;
}

export function formatMonitorScale(monitor: MonitorInfo) {
  if (monitor.scale_factor === null) {
    return "Unavailable";
  }

  const percentage = Math.round(monitor.scale_factor * 100);
  if (monitor.dpi_x === null || monitor.dpi_y === null) {
    return `${percentage}%`;
  }
  if (monitor.dpi_x === monitor.dpi_y) {
    return `${percentage}% (${monitor.dpi_x} DPI)`;
  }
  return `${percentage}% (${monitor.dpi_x} x ${monitor.dpi_y} DPI)`;
}

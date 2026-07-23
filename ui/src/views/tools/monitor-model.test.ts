import { describe, expect, it } from "vitest";

import type { MonitorBounds, MonitorInfo } from "@/lib/runner-api";
import {
  formatMonitorAxisRange,
  formatMonitorScale,
  formatMonitorSize,
} from "@/views/tools/monitor-model";

const negativeBounds: MonitorBounds = {
  bottom: 1080,
  height: 1080,
  left: -1920,
  right: 0,
  top: 0,
  width: 1920,
};

describe("monitor presentation model", () => {
  it("formats dimensions and separate inclusive coordinate ranges", () => {
    expect(formatMonitorSize(negativeBounds)).toBe("1920 x 1080");
    expect(formatMonitorAxisRange(negativeBounds, "x")).toBe("-1920 to -1");
    expect(formatMonitorAxisRange(negativeBounds, "y")).toBe("0 to 1079");
  });

  it("formats uniform and unavailable DPI information", () => {
    expect(formatMonitorScale(monitor({ dpi_x: 120, dpi_y: 120, scale_factor: 1.25 }))).toBe(
      "125% (120 DPI)",
    );
    expect(formatMonitorScale(monitor({ dpi_x: null, dpi_y: null, scale_factor: null }))).toBe(
      "Unavailable",
    );
  });
});

function monitor(overrides: Partial<MonitorInfo>): MonitorInfo {
  return {
    bounds: negativeBounds,
    device_name: String.raw`\\.\DISPLAY2`,
    dpi_x: 96,
    dpi_y: 96,
    id: "windows:display2",
    is_primary: false,
    scale_factor: 1,
    work_area: negativeBounds,
    ...overrides,
  };
}

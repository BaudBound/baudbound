import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { RunLogEntry, TimeFormat } from "@/lib/runner-api";
import {
  createDesktopTimeFormatter,
  DesktopTimeProvider,
} from "@/lib/time-format";
import { RunLogPanel } from "@/views/run-log-panel";

const logs: RunLogEntry[] = [
  {
    action_type: "action.beep",
    level: "info",
    message: "first action",
    node_id: "n-first",
    timestamp_unix_ms: Date.UTC(2026, 6, 17, 8, 15, 0),
  },
  {
    action_type: "action.log",
    level: "info",
    message: "second action",
    node_id: "n-second",
    timestamp_unix_ms: Date.UTC(2026, 6, 17, 20, 45, 0),
  },
];

function renderLogs(timeFormat: TimeFormat) {
  return renderToStaticMarkup(
    <DesktopTimeProvider timeFormat={timeFormat}>
      <RunLogPanel logs={logs} />
    </DesktopTimeProvider>,
  );
}

describe("RunLogPanel", () => {
  it.each(["12-hour", "24-hour"] as const)(
    "renders each emission timestamp with the %s clock",
    (timeFormat) => {
      const markup = renderLogs(timeFormat);
      const formatter = createDesktopTimeFormatter(timeFormat);

      expect(markup).toContain(
        formatter.formatUnixMilliseconds(logs[0].timestamp_unix_ms),
      );
      expect(markup).toContain(
        formatter.formatUnixMilliseconds(logs[1].timestamp_unix_ms),
      );
      expect(markup).toContain("first action");
      expect(markup).toContain("second action");
      expect(markup).toContain("action.beep");
      expect(markup).toContain("action.log");
    },
  );
});

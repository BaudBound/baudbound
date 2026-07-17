import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { RunnerConfig } from "@/lib/runner-api";
import {
  DesktopConfiguration,
  SharedConfiguration,
} from "@/views/configuration-preferences";

const config: RunnerConfig = {
  desktop: {
    keep_running_on_close: true,
    launch_at_login: false,
    start_background_runner_on_launch: false,
    start_minimized_to_tray: false,
  },
  display: { time_format: "24-hour" },
  runner: {
    run_history_max_age_days: 30,
    run_history_max_records: 10_000,
    target_runtimes: [],
    trigger_reload_seconds: 2,
  },
  serial: { devices: {} },
  triggers: {
    file_watch_enabled: true,
    hotkeys_enabled: true,
    process_watch_enabled: true,
    schedules_enabled: true,
    serial_enabled: true,
    startup_enabled: true,
    webhooks_enabled: false,
    websockets_enabled: false,
  },
  updates: { automatic_checks: true, check_interval_hours: 24 },
  webhooks: { bind: "127.0.0.1", max_body_bytes: 1_048_576, port: 43_891 },
  websockets: {
    bind: "127.0.0.1",
    max_connections: 128,
    max_message_bytes: 1_048_576,
    port: 43_892,
  },
};

describe("unified configuration preferences", () => {
  it("labels shared and desktop ownership clearly", () => {
    const markup = renderToStaticMarkup(
      <>
        <DesktopConfiguration
          config={config}
          launchAtLoginRegistered={false}
          onChange={() => undefined}
        />
        <SharedConfiguration config={config} onChange={() => undefined} />
      </>,
    );

    expect(markup).toContain("Desktop configuration");
    expect(markup).toContain("Shared configuration");
    expect(markup).toContain("Automatically check for updates");
    expect(markup).toContain("Clock format");
  });
});

import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { RunnerConfig } from "@/lib/runner-api";
import {
  DesktopConfiguration,
  SharedConfiguration,
} from "@/views/configuration-preferences";

const config: RunnerConfig = {
  desktop: {
    dialog_console_always_on_top: false,
    dialog_console_enabled: false,
    dialog_console_focus_on_request: true,
    keep_running_on_close: true,
    launch_at_login: false,
    start_background_runner_on_launch: false,
    start_minimized_to_tray: false,
  },
  display: { time_format: "24-hour" },
  limits: {
    max_active_runs_global: 16,
    max_active_runs_per_script: 1,
    max_file_download_bytes: 104_857_600,
    max_file_write_bytes_per_run: 1_073_741_824,
    max_file_read_bytes: 10_485_760,
    max_generated_text_bytes: 67_108_864,
    max_http_response_bytes: 10_485_760,
    max_log_entry_bytes: 16_384,
    max_loop_iterations_per_run: 1_000_000,
    max_process_launches_per_minute: 120,
    max_process_output_bytes: 8_388_608,
    max_processes_per_script: 4,
    max_queued_activations_per_script: 64,
    max_schedule_catch_up_events_per_poll: 256,
    max_runtime_variable_bytes: 16_777_216,
    max_run_duration_ms: 3_600_000,
    max_retained_variable_bytes: 262_144,
    max_run_log_bytes: 2_097_152,
    max_run_record_bytes: 8_388_608,
    max_steps_per_run: 1_000_000,
    queue_overflow_strategy: "reject_newest",
  },
  runner: {
    run_history_max_age_days: 30,
    run_history_max_bytes: 1_073_741_824,
    run_history_max_records: 10_000,
    target_runtimes: [],
    trigger_reload_seconds: 2,
  },
  security: {
    policy: {
      allow_dangerous_permissions: true,
      allow_private_http_requests: false,
      allow_public_network_listeners: true,
      allow_shell_commands: true,
    },
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
  webhooks: {
    allow_browser_origins: [],
    allow_unauthenticated_public_bind: false,
    bind: "127.0.0.1",
    body_read_progress_timeout_ms: 10_000,
    body_read_timeout_ms: 30_000,
    header_read_timeout_ms: 10_000,
    max_body_bytes: 1_048_576,
    max_connections: 128,
    max_header_bytes: 32_768,
    max_unauthenticated_connections: 32,
    port: 43_891,
    pre_auth_requests_per_minute_global: 600,
    pre_auth_requests_per_minute_per_address: 60,
    pre_auth_timeout_ms: 5_000,
  },
  websockets: {
    allow_browser_origins: [],
    allow_unauthenticated_public_bind: false,
    bind: "127.0.0.1",
    handshake_timeout_ms: 5_000,
    max_connections: 128,
    max_message_bytes: 1_048_576,
    max_unauthenticated_connections: 32,
    port: 43_892,
    pre_auth_requests_per_minute_global: 600,
    pre_auth_requests_per_minute_per_address: 60,
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
    expect(markup).toContain("Dialog console mode");
  });
});

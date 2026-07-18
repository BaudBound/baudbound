import { invoke } from "@tauri-apps/api/core";

export type PackageHashStatus =
  | { state: "valid" }
  | { actual: string; expected: string; state: "mismatch" }
  | { message?: string; state: "error" };

export type ApprovalStatus =
  | { state: "current" }
  | { state: "missing" }
  | { state: "package_unavailable" }
  | { state: "permission_mismatch" }
  | { state: "unknown" }
  | { message?: string; state: "error" }
  | {
      approved_package_hash: string;
      installed_package_hash: string;
      state: "stale_package_hash";
    };

export type InstalledScript = {
  asset_count: number;
  enabled: boolean;
  id: string;
  imported_at_unix: number;
  name: string;
  package_file_name: string;
  package_format_version: number;
  package_hash: string;
  package_path: string;
  risk_level: string;
  script_language_version: number;
  target_runtime: string;
};

export type TriggerRegistrationStatus = {
  action_type: string;
  device_id: string | null;
  node_id: string;
  runner_type: string;
  target: string | null;
};

export type NetworkTriggerType = "webhook" | "websocket";

export type TriggerAuthStatus = {
  auth_enabled: boolean;
  created_at_unix: number;
  disabled_at_unix: number | null;
  node_id: string;
  rotated_at_unix: number | null;
  script_id: string;
  token_preview: string;
  trigger_type: NetworkTriggerType;
};

export type GeneratedTriggerToken = {
  status: TriggerAuthStatus;
  token: string;
};

export type ScriptStatus = {
  approval_status: ApprovalStatus;
  declared_permissions: string[];
  installed: InstalledScript;
  package_error: string | null;
  package_hash_status: PackageHashStatus;
  triggers: TriggerRegistrationStatus[];
};

export type RunnerStatus = {
  disabled_script_count: number;
  enabled_script_count: number;
  problem_count: number;
  scripts: ScriptStatus[];
  supported_target_runtimes: string[];
  total_script_count: number;
  trigger_count: number;
};

export type DesktopBackgroundRunnerState = {
  message: string;
  running: boolean;
  started_at_unix: number | null;
  state: "failed" | "running" | "stopped" | "stopping";
  stopped_at_unix: number | null;
};

export type SerialDeviceStatus = {
  auto_reconnect: boolean;
  auto_rebind_port: boolean;
  baud_rate: number;
  data_bits: number;
  device_id: string;
  flow_control: string;
  manufacturer: string | null;
  parity: string;
  port: string;
  product_id: string | null;
  product: string | null;
  read_mode: string;
  serial_number: string | null;
  stop_bits: string;
  validate_usb_identity: boolean;
  vendor_id: string | null;
};

export type SerialReaderStatus = {
  auto_reconnect: boolean;
  auto_rebind_port: boolean;
  device_id: string;
  last_error: string | null;
  last_error_unix: number | null;
  last_event_unix: number | null;
  node_id: string;
  port: string;
  script_id: string;
  state: string;
};

export type SerialPortScanResult = {
  manufacturer: string | null;
  port: string;
  port_type: string;
  product: string | null;
  product_id: string | null;
  serial_number: string | null;
  vendor_id: string | null;
};

export type MonitorBounds = {
  bottom: number;
  height: number;
  left: number;
  right: number;
  top: number;
  width: number;
};

export type MonitorInfo = {
  bounds: MonitorBounds;
  device_name: string;
  dpi_x: number | null;
  dpi_y: number | null;
  id: string;
  is_primary: boolean;
  scale_factor: number | null;
  work_area: MonitorBounds;
};

export type MonitorDiscoveryResult = {
  monitors: MonitorInfo[];
  supported: boolean;
  unavailable_reason: string | null;
  virtual_bounds: MonitorBounds | null;
};

export type ScreenPixel = {
  alpha: number;
  blue: number;
  green: number;
  hex: string;
  integer: number;
  red: number;
};

export type CoordinatePickerResult = {
  color: ScreenPixel;
  monitor: MonitorInfo;
  x: number;
  y: number;
};

export type CoordinatePickerEvent =
  | { result: CoordinatePickerResult; status: "selected" }
  | { status: "cancelled" }
  | { message: string; status: "failed" };

export type CoordinatePickerStartPayload = {
  monitor_count: number;
  session_id: string;
};

export type ServiceStatusService = {
  active: boolean;
  diagnostics?: TriggerServiceDiagnostics;
  details?: {
    readers?: SerialReaderStatus[];
  };
  enabled: boolean;
  name: string;
  registrations: number;
  target: string;
};

export type TriggerServiceDiagnostics = {
  running: boolean;
  state: "active" | "idle" | "stopped" | string;
  summary: string;
};

export type DispatchActivity = {
  completed_at_unix: number;
  error: string | null;
  node_id: string;
  run_id: string | null;
  script_id: string;
  source: string;
  status: "completed" | "failed";
};

export type ServiceActivity = {
  failed_dispatch_count: number;
  last_dispatch: DispatchActivity | null;
  total_dispatch_count: number;
  triggers: Record<string, TriggerDispatchActivity>;
};

export type TriggerDispatchActivity = {
  failed_dispatch_count: number;
  last_dispatch: DispatchActivity | null;
  last_failure_unix: number | null;
  last_success_unix: number | null;
  successful_dispatch_count: number;
  total_dispatch_count: number;
};

export type ServiceStatusDocument = {
  active_service_count: number;
  activity: ServiceActivity;
  configured_serial_device_count: number;
  idle: boolean;
  last_heartbeat_unix: number;
  last_reload_unix: number;
  pid: number;
  reload_interval_seconds: number;
  services: ServiceStatusService[];
  started_at_unix: number;
  state: string;
  storage_root: string;
  time_format: TimeFormat;
};

export type ServiceHealthDocument = {
  health: string;
  heartbeat_age_seconds?: number;
  ok: boolean;
  reason: string;
  stale: boolean;
  stale_after_seconds?: number;
};

export type NativeDoctorCheck = {
  action_types: string[];
  available: boolean;
  label: string;
  note: string;
};

export type RunLogEntry = {
  action_type?: string | null;
  level: string;
  message: string;
  node_id?: string | null;
  timestamp_unix_ms: number;
};

export type ActiveRun = {
  cancellation_requested: boolean;
  discarded_log_count: number;
  logs: RunLogEntry[];
  run_id: string;
  script_id: string;
  started_at_unix_ms: number;
  trigger_node_id: string;
};

export type ActiveRunEvent =
  | { kind: "started"; revision: number; run: ActiveRun }
  | {
      kind: "log_emitted";
      discarded_log_count: number;
      log: RunLogEntry;
      revision: number;
      run_id: string;
    }
  | { kind: "cancellation_requested"; revision: number; run_id: string }
  | { kind: "finished"; revision: number; run_id: string }
  | { kind: "run_recorded"; revision: number };

export type StoredRunRecord = {
  completed_at_unix: number;
  logs: RunLogEntry[];
  run_id: string;
  script_id: string;
  status: "cancelled" | "completed" | "failed";
  trigger_node_id: string;
  variables: Record<string, unknown>;
};

export type DashboardPayload = {
  active_runs: ActiveRun[];
  active_runs_revision: number;
  automatic_update_checks: boolean;
  config_path: string;
  desktop_background: DesktopBackgroundRunnerState;
  launch_at_login_desired: boolean;
  launch_at_login_registered: boolean | null;
  native_doctor_checks: NativeDoctorCheck[];
  recent_runs: StoredRunRecord[];
  runner: RunnerStatus;
  secret_storage_available: boolean;
  secret_statuses: Record<string, InstalledSecretStatus[]>;
  serial_devices: SerialDeviceStatus[];
  service_health: ServiceHealthDocument;
  service_status: ServiceStatusDocument | null;
  storage_root: string;
  time_format: TimeFormat;
  trigger_auth_statuses: Record<string, TriggerAuthStatus[]>;
};

export type InstalledSecretStatus = {
  configured: boolean;
  description: string;
  name: string;
  required: boolean;
  updated_at_unix: number | null;
  value_type: string;
};

export type ActionPayload = {
  dashboard: DashboardPayload;
  generated_trigger_tokens?: GeneratedTriggerToken[];
  message: string;
};

export type PackageActionPayload = ActionPayload & {
  generated_trigger_tokens: GeneratedTriggerToken[];
};

export type GeneratedTriggerTokenPayload = ActionPayload & {
  status: TriggerAuthStatus;
  token: string;
};

export type TimeFormat = "12-hour" | "24-hour";

export type DisplaySettings = {
  time_format: TimeFormat;
};

export type DesktopSettings = {
  keep_running_on_close: boolean;
  launch_at_login: boolean;
  start_background_runner_on_launch: boolean;
  start_minimized_to_tray: boolean;
};

export type UpdateSettings = {
  automatic_checks: boolean;
  check_interval_hours: number;
};

export type RunnerConfig = {
  desktop: DesktopSettings;
  display: DisplaySettings;
  limits: LimitSettings;
  runner: RunnerSettings;
  serial: SerialSettings;
  triggers: TriggerSettings;
  updates: UpdateSettings;
  webhooks: WebhookSettings;
  websockets: WebSocketSettings;
};

export type LimitSettings = {
  max_file_download_bytes: number;
  max_file_read_bytes: number;
  max_http_response_bytes: number;
};

export type RunnerSettings = {
  run_history_max_age_days: number;
  run_history_max_records: number;
  target_runtimes: string[];
  trigger_reload_seconds: number;
};

export type TriggerSettings = {
  file_watch_enabled: boolean;
  hotkeys_enabled: boolean;
  process_watch_enabled: boolean;
  schedules_enabled: boolean;
  serial_enabled: boolean;
  startup_enabled: boolean;
  webhooks_enabled: boolean;
  websockets_enabled: boolean;
};

export type SerialSettings = {
  devices: Record<string, SerialDeviceSettings>;
};

export type SerialDeviceSettings = {
  auto_reconnect: boolean;
  auto_rebind_port: boolean;
  baud_rate: number;
  data_bits: number;
  flow_control: string;
  manufacturer: string | null;
  parity: string;
  port: string;
  product_id: string | null;
  product: string | null;
  read_mode: string;
  serial_number: string | null;
  stop_bits: string;
  validate_usb_identity: boolean;
  vendor_id: string | null;
};

export type WebhookSettings = {
  allow_browser_origins: string[];
  allow_unauthenticated_public_bind: boolean;
  bind: string;
  max_body_bytes: number;
  port: number;
};

export type WebSocketSettings = {
  allow_browser_origins: string[];
  allow_unauthenticated_public_bind: boolean;
  bind: string;
  max_connections: number;
  max_message_bytes: number;
  port: number;
};

export type RunnerConfigPayload = {
  config: RunnerConfig;
  contents: string;
  launch_at_login_registered: boolean;
  path: string;
};

type ConfirmationChallenge = {
  confirmation_id: string;
  expires_at_unix_ms: number;
  operation_kind: string;
  summary: string;
};

type SensitiveOperation = { kind: string } & Record<string, unknown>;

async function invokeSensitive<T>(
  command: string,
  operation: SensitiveOperation,
  args: Record<string, unknown>,
) {
  const challenge = await invoke<ConfirmationChallenge>("prepare_sensitive_operation", {
    operation,
  });
  return invoke<T>(command, { ...args, confirmationId: challenge.confirmation_id });
}

export function getDashboardState() {
  return invoke<DashboardPayload>("dashboard_state");
}

export function readRunnerConfig() {
  return invoke<RunnerConfigPayload>("read_runner_config");
}

export function shouldCheckForUpdate() {
  return invoke<boolean>("should_check_for_update");
}

export function recordUpdateCheck(latestVersion: string | null, releaseNotes: string | null) {
  return invoke<void>("record_update_check", { latestVersion, releaseNotes });
}

export function saveRunnerConfig(contents: string, restartBackground: boolean) {
  return invokeSensitive<ActionPayload>(
    "save_runner_config",
    { kind: "save_runner_config", contents, restart_background: restartBackground },
    { contents, restartBackground },
  );
}

export function saveRunnerConfigModel(config: RunnerConfig, restartBackground: boolean) {
  return invokeSensitive<ActionPayload>(
    "save_runner_config_model",
    { kind: "save_runner_config_model", config, restart_background: restartBackground },
    { config, restartBackground },
  );
}

export function resetRunnerConfig(restartBackground: boolean) {
  return invokeSensitive<ActionPayload>(
    "reset_runner_config",
    { kind: "reset_runner_config", restart_background: restartBackground },
    { restartBackground },
  );
}

export function scanSerialPorts() {
  return invoke<SerialPortScanResult[]>("scan_serial_ports").catch((error) => {
    const message = String(error);
    if (message.includes("Command scan_serial_ports not found")) {
      throw new Error(
        "Serial scanning requires the latest desktop backend. Close BaudBound and start the desktop app again so the new Rust command is available.",
      );
    }
    throw error;
  });
}

export function discoverMonitors() {
  return invoke<MonitorDiscoveryResult>("discover_monitors").catch((error) => {
    const message = String(error);
    if (message.includes("Command discover_monitors not found")) {
      throw new Error(
        "Monitor discovery requires the latest desktop backend. Close BaudBound and start the desktop app again so the new Rust command is available.",
      );
    }
    throw error;
  });
}

export function startCoordinatePicker() {
  return invoke<CoordinatePickerStartPayload>("start_coordinate_picker").catch((error) => {
    const message = String(error);
    if (message.includes("Command start_coordinate_picker not found")) {
      throw new Error(
        "The coordinate picker requires the latest desktop backend. Close BaudBound and start the desktop app again so the new Rust command is available.",
      );
    }
    throw error;
  });
}

export function selectCoordinatePicker(sessionId: string) {
  return invoke<void>("select_coordinate_picker", { sessionId });
}

export function cancelCoordinatePicker(sessionId: string) {
  return invoke<void>("cancel_coordinate_picker", { sessionId });
}

export function selectPackageFile() {
  return invoke<string | null>("select_package_file");
}

export function approveScript(reference: string) {
  return invokeSensitive<PackageActionPayload>(
    "approve_script",
    { kind: "approve_script", reference },
    { reference },
  );
}

export function revokeScriptApproval(reference: string) {
  return invoke<ActionPayload>("revoke_script_approval", { reference });
}

export function importScriptPackage(packagePath: string) {
  return invokeSensitive<ActionPayload>(
    "import_script_package",
    { kind: "import_script_package", package_path: packagePath },
    { packagePath },
  );
}

export function updateScriptPackage(packagePath: string) {
  return invokeSensitive<ActionPayload>(
    "update_script_package",
    { kind: "update_script_package", package_path: packagePath },
    { packagePath },
  );
}

export function startBackgroundRunner() {
  return invoke<ActionPayload>("start_background_runner");
}

export function reloadBackgroundRunner() {
  return invoke<ActionPayload>("reload_background_runner");
}

export function stopBackgroundRunner() {
  return invoke<ActionPayload>("stop_background_runner");
}

export function prepareForUpdate() {
  return invoke<ActionPayload>("prepare_for_update");
}

export function removeScript(reference: string) {
  return invoke<ActionPayload>("remove_script", { reference });
}

export function runScript(reference: string) {
  return invokeSensitive<ActionPayload>(
    "run_script",
    { kind: "run_script", reference },
    { reference },
  );
}

export function stopRun(runId: string) {
  return invoke<ActionPayload>("stop_run", { runId });
}

export function stopScriptRuns(reference: string) {
  return invoke<ActionPayload>("stop_script_runs", { reference });
}

export function setScriptEnabled(reference: string, enabled: boolean) {
  return invoke<ActionPayload>("set_script_enabled", { enabled, reference });
}

export function rotateNetworkTriggerToken(
  reference: string,
  nodeId: string,
  triggerType: NetworkTriggerType,
) {
  return invokeSensitive<GeneratedTriggerTokenPayload>(
    "rotate_network_trigger_token",
    {
      kind: "rotate_network_trigger_token",
      node_id: nodeId,
      reference,
      trigger_type: triggerType,
    },
    { nodeId, reference, triggerType },
  );
}

export function setNetworkTriggerAuthEnabled(
  reference: string,
  nodeId: string,
  triggerType: NetworkTriggerType,
  enabled: boolean,
) {
  return invokeSensitive<ActionPayload>(
    "set_network_trigger_auth_enabled",
    {
      enabled,
      kind: "set_network_trigger_auth_enabled",
      node_id: nodeId,
      reference,
      trigger_type: triggerType,
    },
    { request: { enabled, nodeId, reference, triggerType } },
  );
}

export function setScriptSecret(reference: string, name: string, value: string) {
  return invokeSensitive<ActionPayload>(
    "set_script_secret",
    { kind: "set_script_secret", name, reference, value },
    { name, reference, value },
  );
}

export function removeScriptSecret(reference: string, name: string) {
  return invokeSensitive<ActionPayload>(
    "remove_script_secret",
    { kind: "remove_script_secret", name, reference },
    { name, reference },
  );
}

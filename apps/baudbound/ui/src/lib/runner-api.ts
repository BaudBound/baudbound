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
  runner_name: string;
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
  runner_name: string;
  services: ServiceStatusService[];
  started_at_unix: number;
  state: string;
  storage_root: string;
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
  level: string;
  message: string;
  node_id?: string | null;
};

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
  config_path: string;
  desktop_background: DesktopBackgroundRunnerState;
  native_doctor_checks: NativeDoctorCheck[];
  recent_runs: StoredRunRecord[];
  runner: RunnerStatus;
  secret_statuses: Record<string, InstalledSecretStatus[]>;
  serial_devices: SerialDeviceStatus[];
  service_health: ServiceHealthDocument;
  service_status: ServiceStatusDocument | null;
  storage_root: string;
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
  message: string;
};

export type RunnerConfig = {
  runner: RunnerSettings;
  serial: SerialSettings;
  triggers: TriggerSettings;
  webhooks: WebhookSettings;
  websockets: WebSocketSettings;
};

export type RunnerSettings = {
  name: string | null;
  run_history_max_age_days: number;
  run_history_max_records: number;
  target_runtimes: string[];
  trigger_reload_seconds: number;
};

export type TriggerSettings = {
  file_watch_enabled: boolean;
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
  bind: string;
  max_body_bytes: number;
  port: number;
};

export type WebSocketSettings = {
  bind: string;
  max_connections: number;
  max_message_bytes: number;
  port: number;
};

export type RunnerConfigPayload = {
  config: RunnerConfig;
  contents: string;
  path: string;
};

export function getDashboardState() {
  return invoke<DashboardPayload>("dashboard_state");
}

export function readRunnerConfig() {
  return invoke<RunnerConfigPayload>("read_runner_config");
}

export function saveRunnerConfig(contents: string, restartBackground: boolean) {
  return invoke<ActionPayload>("save_runner_config", { contents, restartBackground });
}

export function saveRunnerConfigModel(config: RunnerConfig, restartBackground: boolean) {
  return invoke<ActionPayload>("save_runner_config_model", { config, restartBackground });
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

export function selectPackageFile() {
  return invoke<string | null>("select_package_file");
}

export function approveScript(reference: string) {
  return invoke<ActionPayload>("approve_script", { reference });
}

export function revokeScriptApproval(reference: string) {
  return invoke<ActionPayload>("revoke_script_approval", { reference });
}

export function importScriptPackage(packagePath: string) {
  return invoke<ActionPayload>("import_script_package", { packagePath });
}

export function updateScriptPackage(packagePath: string) {
  return invoke<ActionPayload>("update_script_package", { packagePath });
}

export function requestTriggerReload() {
  return invoke<ActionPayload>("request_trigger_reload");
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
  return invoke<ActionPayload>("run_script", { reference });
}

export function setScriptEnabled(reference: string, enabled: boolean) {
  return invoke<ActionPayload>("set_script_enabled", { enabled, reference });
}

export function setScriptSecret(reference: string, name: string, value: string) {
  return invoke<ActionPayload>("set_script_secret", { name, reference, value });
}

export function removeScriptSecret(reference: string, name: string) {
  return invoke<ActionPayload>("remove_script_secret", { name, reference });
}

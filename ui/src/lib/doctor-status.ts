import type { DashboardPayload } from "@/lib/runner-api";

export type DoctorCheckState = "idle" | "ok" | "warn";

export type DoctorStatus = {
  missingSerialDeviceIds: string[];
  referencedSerialDeviceCount: number;
  states: {
    desktopBackground: DoctorCheckState;
    enabledScripts: DoctorCheckState;
    installedScripts: DoctorCheckState;
    launchAtLogin: DoctorCheckState;
    runHistory: DoctorCheckState;
    runtimeSupport: DoctorCheckState;
    securityReview: DoctorCheckState;
    serialDeviceConfig: DoctorCheckState;
  };
  warningCount: number;
};

export function doctorStatus(dashboard: DashboardPayload): DoctorStatus {
  const configuredSerialDevices = new Set(dashboard.serial_devices.map((device) => device.device_id));
  const referencedSerialDevices = new Set(
    dashboard.runner.scripts.flatMap((script) =>
      script.triggers
        .filter((trigger) => trigger.action_type === "trigger.serial_input")
        .map((trigger) => trigger.device_id)
        .filter(isNonEmptyString),
    ),
  );
  const missingSerialDeviceIds = [...referencedSerialDevices].filter(
    (deviceId) => !configuredSerialDevices.has(deviceId),
  );
  const states: DoctorStatus["states"] = {
    desktopBackground: dashboard.desktop_background.running ? "ok" : "idle",
    enabledScripts: dashboard.runner.enabled_script_count > 0 ? "ok" : "warn",
    installedScripts: dashboard.runner.total_script_count > 0 ? "ok" : "idle",
    launchAtLogin:
      dashboard.launch_at_login_registered === null ||
      dashboard.launch_at_login_desired !== dashboard.launch_at_login_registered
        ? "warn"
        : "ok",
    runHistory: dashboard.run_statistics.total > 0 ? "ok" : "idle",
    runtimeSupport: dashboard.runner.supported_target_runtimes.length > 0 ? "ok" : "warn",
    securityReview: dashboard.runner.problem_count > 0 ? "warn" : "ok",
    serialDeviceConfig:
      missingSerialDeviceIds.length > 0
        ? "warn"
        : referencedSerialDevices.size > 0 || dashboard.serial_devices.length > 0
          ? "ok"
          : "idle",
  };

  return {
    missingSerialDeviceIds,
    referencedSerialDeviceCount: referencedSerialDevices.size,
    states,
    warningCount: Object.values(states).filter((state) => state === "warn").length,
  };
}

function isNonEmptyString(value: string | null): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

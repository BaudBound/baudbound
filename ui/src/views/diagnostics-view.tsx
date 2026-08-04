import { Details } from "@/components/details";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { formatCount } from "@/lib/count-format";
import { doctorStatus, type DoctorCheckState, type DoctorStatus } from "@/lib/doctor-status";
import type { DashboardPayload, NativeDoctorCheck } from "@/lib/runner-api";
import { TriggerRegistrationPanel } from "@/views/diagnostics/trigger-registration-panel";

type DoctorCheck = {
  detail: string;
  label: string;
  state: DoctorCheckState;
};

export function DiagnosticsView({ dashboard }: { dashboard: DashboardPayload }) {
  const nativeDoctorChecks = dashboard.native_doctor_checks ?? [];
  const status = doctorStatus(dashboard);
  const checks = doctorChecks(dashboard, status);
  const warningCount = status.warningCount;
  const idleCount = checks.filter((check) => check.state === "idle").length;
  const unsupportedNativeCount = nativeDoctorChecks.filter(
    (check) => !check.available,
  ).length;

  return (
    <div className="grid gap-4">
      <Card>
        <CardHeader className="flex flex-row items-center justify-between gap-3">
          <CardTitle>Doctor checks</CardTitle>
          <Badge variant={warningCount > 0 ? "medium" : idleCount > 0 ? "muted" : "good"}>
            {warningCount > 0
              ? formatCount(warningCount, "warning")
              : idleCount > 0
                ? `${idleCount} idle`
                : "Ready"}
          </Badge>
        </CardHeader>
        <CardContent className="grid gap-3 md:grid-cols-2">
          {checks.map((check) => (
            <DoctorCheckCard check={check} key={check.label} />
          ))}
        </CardContent>
      </Card>

      <Card>
        <CardHeader className="flex flex-row items-center justify-between gap-3">
          <CardTitle>Native desktop action support</CardTitle>
          <Badge variant={unsupportedNativeCount > 0 ? "medium" : "good"}>
            {unsupportedNativeCount > 0 ? `${unsupportedNativeCount} unsupported` : "Supported"}
          </Badge>
        </CardHeader>
        <CardContent className="grid gap-3 lg:grid-cols-2">
          {nativeDoctorChecks.length > 0 ? (
            nativeDoctorChecks.map((check) => (
              <NativeDoctorCard check={check} key={check.label} />
            ))
          ) : (
            <div className="rounded-md border border-border bg-background p-3 text-sm text-muted-foreground lg:col-span-2">
              The current runner backend does not provide native desktop action checks.
            </div>
          )}
        </CardContent>
      </Card>

      <TriggerRegistrationPanel dashboard={dashboard} />

      <div className="grid gap-4 xl:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Paths</CardTitle>
          </CardHeader>
          <CardContent>
            <Details
              rows={[
                ["Runner home", dashboard.storage_root],
                ["Config file", dashboard.config_path],
              ]}
            />
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Runtime facts</CardTitle>
          </CardHeader>
          <CardContent>
            <Details
              rows={[
                ["Desktop loop", dashboard.desktop_background.state],
                ["Target runtimes", dashboard.runner.supported_target_runtimes.join(", ")],
                ["Retained run records", dashboard.run_statistics.total.toString()],
              ]}
            />
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

function NativeDoctorCard({ check }: { check: NativeDoctorCheck }) {
  return (
    <div className="rounded-md border border-border bg-background p-3">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="text-sm font-medium">{check.label}</div>
          <div className="mt-1 text-xs text-muted-foreground">{check.note}</div>
        </div>
        <Badge variant={check.available ? "good" : "medium"}>
          {check.available ? "Supported" : "Unsupported"}
        </Badge>
      </div>
      <div className="mt-3 flex flex-wrap gap-1">
        {check.action_types.map((actionType) => (
          <Badge key={actionType} variant="muted">
            {actionType}
          </Badge>
        ))}
      </div>
    </div>
  );
}

function DoctorCheckCard({ check }: { check: DoctorCheck }) {
  return (
    <div className="rounded-md border border-border bg-background p-3">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="text-sm font-medium">{check.label}</div>
          <div className="mt-1 text-xs text-muted-foreground">{check.detail}</div>
        </div>
        <Badge variant={checkVariant(check.state)}>{checkLabel(check.state)}</Badge>
      </div>
    </div>
  );
}

function doctorChecks(dashboard: DashboardPayload, status: DoctorStatus): DoctorCheck[] {
  const hasScripts = dashboard.runner.total_script_count > 0;
  const hasEnabledScripts = dashboard.runner.enabled_script_count > 0;
  const hasTargetRuntimes = dashboard.runner.supported_target_runtimes.length > 0;
  const hasRunRecords = dashboard.run_statistics.total > 0;
  const needsReview = dashboard.runner.problem_count > 0;
  const serialDevices = dashboard.serial_devices ?? [];
  const serialDeviceDetail =
    status.missingSerialDeviceIds.length > 0
      ? `Missing runner config for ${status.missingSerialDeviceIds.join(", ")}.`
      : status.referencedSerialDeviceCount > 0
        ? `${formatCount(status.referencedSerialDeviceCount, "serial device ID")} ${
            status.referencedSerialDeviceCount === 1 ? "is" : "are"
          } referenced by installed scripts.`
        : serialDevices.length > 0
          ? `${formatCount(serialDevices.length, "local serial device configuration")} ${
              serialDevices.length === 1 ? "is" : "are"
            } available.`
          : "No serial devices are configured or referenced.";

  return [
    {
      detail:
        dashboard.launch_at_login_registered === null
          ? "The operating system launch-at-login registration could not be inspected."
          : dashboard.launch_at_login_desired === dashboard.launch_at_login_registered
            ? dashboard.launch_at_login_registered
              ? "Launch at login is enabled and registered with the operating system."
              : "Launch at login is disabled."
            : "The configuration and operating system registration do not match. Save the configuration to repair the registration.",
      label: "Launch at login",
      state: status.states.launchAtLogin,
    },
    {
      detail: dashboard.desktop_background.running
        ? dashboard.desktop_background.message
        : "Listener triggers will not fire until the desktop background runner is started.",
      label: "Desktop background runner",
      state: status.states.desktopBackground,
    },
    {
      detail: hasScripts
        ? `${formatCount(dashboard.runner.total_script_count, "installed script")} ${
            dashboard.runner.total_script_count === 1 ? "was" : "were"
          } found.`
        : "Install a .bbs package before the runner can execute scripts.",
      label: "Installed scripts",
      state: status.states.installedScripts,
    },
    {
      detail: hasEnabledScripts
        ? `${formatCount(dashboard.runner.enabled_script_count, "script")} ${
            dashboard.runner.enabled_script_count === 1 ? "is" : "are"
          } enabled.`
        : "No scripts are enabled.",
      label: "Enabled scripts",
      state: status.states.enabledScripts,
    },
    {
      detail: needsReview
        ? `${formatCount(dashboard.runner.problem_count, "script")} ${
            dashboard.runner.problem_count === 1 ? "needs" : "need"
          } approval or package review.`
        : "No approval or package hash issues are visible.",
      label: "Security review",
      state: status.states.securityReview,
    },
    {
      detail: hasTargetRuntimes
        ? dashboard.runner.supported_target_runtimes.join(", ")
        : "No target runtimes are currently reported.",
      label: "Runtime support",
      state: status.states.runtimeSupport,
    },
    {
      detail: serialDeviceDetail,
      label: "Serial device config",
      state: status.states.serialDeviceConfig,
    },
    {
      detail: hasRunRecords
        ? `${formatCount(dashboard.run_statistics.total, "retained run record")} ${
            dashboard.run_statistics.total === 1 ? "is" : "are"
          } available.`
        : "Run history will appear after scripts execute.",
      label: "Run history",
      state: status.states.runHistory,
    },
  ];
}

function checkVariant(state: DoctorCheckState) {
  if (state === "ok") return "good";
  if (state === "warn") return "medium";
  return "muted";
}

function checkLabel(state: DoctorCheckState) {
  if (state === "ok") return "OK";
  if (state === "warn") return "Review";
  return "Idle";
}

import {
  AlertTriangle,
  CheckCircle2,
  CircleDashed,
  Cpu,
  FolderCog,
  HardDrive,
  LogIn,
  MonitorCog,
  ShieldCheck,
  Stethoscope,
} from "lucide-react";
import type { ReactNode } from "react";

import { Details } from "@/components/details";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { DashboardPayload, NativeDoctorCheck } from "@/lib/runner-api";
import { TriggerRegistrationPanel } from "@/views/diagnostics/trigger-registration-panel";

type CheckState = "ok" | "warn" | "idle";

type DoctorCheck = {
  detail: string;
  icon: ReactNode;
  label: string;
  state: CheckState;
};

export function DiagnosticsView({ dashboard }: { dashboard: DashboardPayload }) {
  const nativeDoctorChecks = dashboard.native_doctor_checks ?? [];
  const checks = doctorChecks(dashboard);
  const warningCount = checks.filter((check) => check.state === "warn").length;
  const idleCount = checks.filter((check) => check.state === "idle").length;
  const unsupportedNativeCount = nativeDoctorChecks.filter(
    (check) => !check.available,
  ).length;

  return (
    <div className="grid gap-4">
      <Card>
        <CardHeader className="flex flex-row items-center justify-between gap-3">
          <div className="flex min-w-0 items-center gap-2">
            <Stethoscope className="size-4 text-muted-foreground" />
            <CardTitle>Doctor checks</CardTitle>
          </div>
          <Badge variant={warningCount > 0 ? "medium" : idleCount > 0 ? "muted" : "good"}>
            {warningCount > 0
              ? `${warningCount} warnings`
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
          <div className="flex min-w-0 items-center gap-2">
            <Cpu className="size-4 text-muted-foreground" />
            <CardTitle>Native desktop action support</CardTitle>
          </div>
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
              Native desktop action checks are not available from the current runner backend yet.
            </div>
          )}
        </CardContent>
      </Card>

      <TriggerRegistrationPanel dashboard={dashboard} />

      <div className="grid gap-4 xl:grid-cols-2">
        <Card>
          <CardHeader className="flex flex-row items-center gap-2">
            <FolderCog className="size-4 text-muted-foreground" />
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
          <CardHeader className="flex flex-row items-center gap-2">
            <MonitorCog className="size-4 text-muted-foreground" />
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
        <div className="flex min-w-0 gap-2">
          <div className="mt-0.5 text-muted-foreground">{check.icon}</div>
          <div className="min-w-0">
            <div className="text-sm font-medium">{check.label}</div>
            <div className="mt-1 text-xs text-muted-foreground">{check.detail}</div>
          </div>
        </div>
        <Badge variant={checkVariant(check.state)}>{checkLabel(check.state)}</Badge>
      </div>
    </div>
  );
}

function doctorChecks(dashboard: DashboardPayload): DoctorCheck[] {
  const hasScripts = dashboard.runner.total_script_count > 0;
  const hasEnabledScripts = dashboard.runner.enabled_script_count > 0;
  const hasTargetRuntimes = dashboard.runner.supported_target_runtimes.length > 0;
  const hasRunRecords = dashboard.run_statistics.total > 0;
  const needsReview = dashboard.runner.problem_count > 0;
  const serialDevices = dashboard.serial_devices ?? [];
  const scripts = dashboard.runner.scripts ?? [];
  const configuredSerialDevices = new Set(
    serialDevices.map((device) => device.device_id),
  );
  const referencedSerialDevices = new Set(
    scripts.flatMap((script) =>
      script.triggers
        .filter((trigger) => trigger.action_type === "trigger.serial_input")
        .map((trigger) => trigger.device_id)
        .filter(isNonEmptyString),
    ),
  );
  const missingSerialDevices = Array.from(referencedSerialDevices).filter(
    (deviceId) => !configuredSerialDevices.has(deviceId),
  );
  const serialDeviceDetail =
    missingSerialDevices.length > 0
      ? `Missing runner config for ${missingSerialDevices.join(", ")}.`
      : referencedSerialDevices.size > 0
        ? `${referencedSerialDevices.size} serial device ID${referencedSerialDevices.size === 1 ? "" : "s"} referenced by installed scripts.`
        : serialDevices.length > 0
          ? `${serialDevices.length} local serial device config${serialDevices.length === 1 ? "" : "s"} available.`
          : "No serial devices are configured or referenced.";

  return [
    {
      detail:
        dashboard.launch_at_login_registered === null
          ? "The operating system login startup registration could not be inspected."
          : dashboard.launch_at_login_desired === dashboard.launch_at_login_registered
            ? dashboard.launch_at_login_registered
              ? "Login startup is enabled and registered with the operating system."
              : "Login startup is disabled."
            : "The TOML setting and operating system registration do not match. Save Config to repair it.",
      icon: <LogIn className="size-4" />,
      label: "Login startup registration",
      state:
        dashboard.launch_at_login_registered === null ||
        dashboard.launch_at_login_desired !== dashboard.launch_at_login_registered
          ? "warn"
          : "ok",
    },
    {
      detail: dashboard.desktop_background.running
        ? dashboard.desktop_background.message
        : "Listener triggers will not fire until the desktop background runner is started.",
      icon: <MonitorCog className="size-4" />,
      label: "Desktop background runner",
      state: dashboard.desktop_background.running ? "ok" : "idle",
    },
    {
      detail: hasScripts
        ? `${dashboard.runner.total_script_count} installed script${dashboard.runner.total_script_count === 1 ? "" : "s"} found.`
        : "Install a .bbs package before the runner can execute scripts.",
      icon: <HardDrive className="size-4" />,
      label: "Installed scripts",
      state: hasScripts ? "ok" : "idle",
    },
    {
      detail: hasEnabledScripts
        ? `${dashboard.runner.enabled_script_count} script${dashboard.runner.enabled_script_count === 1 ? "" : "s"} enabled.`
        : "All installed scripts are disabled.",
      icon: <CheckCircle2 className="size-4" />,
      label: "Enabled scripts",
      state: hasEnabledScripts ? "ok" : "warn",
    },
    {
      detail: needsReview
        ? `${dashboard.runner.problem_count} script${dashboard.runner.problem_count === 1 ? "" : "s"} need approval or package review.`
        : "No approval or package hash issues are visible.",
      icon: <ShieldCheck className="size-4" />,
      label: "Security review",
      state: needsReview ? "warn" : "ok",
    },
    {
      detail: hasTargetRuntimes
        ? dashboard.runner.supported_target_runtimes.join(", ")
        : "No target runtimes are currently reported.",
      icon: <CircleDashed className="size-4" />,
      label: "Runtime support",
      state: hasTargetRuntimes ? "ok" : "warn",
    },
    {
      detail: serialDeviceDetail,
      icon: <HardDrive className="size-4" />,
      label: "Serial device config",
      state:
        missingSerialDevices.length > 0
          ? "warn"
          : referencedSerialDevices.size > 0 || serialDevices.length > 0
            ? "ok"
            : "idle",
    },
    {
      detail: hasRunRecords
        ? `${dashboard.run_statistics.total} retained run record${dashboard.run_statistics.total === 1 ? "" : "s"} available.`
        : "Run history will appear after scripts execute.",
      icon: <AlertTriangle className="size-4" />,
      label: "Run history",
      state: hasRunRecords ? "ok" : "idle",
    },
  ];
}

function checkVariant(state: CheckState) {
  if (state === "ok") return "good";
  if (state === "warn") return "medium";
  return "muted";
}

function checkLabel(state: CheckState) {
  if (state === "ok") return "OK";
  if (state === "warn") return "Review";
  return "Idle";
}

function isNonEmptyString(value: string | null): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

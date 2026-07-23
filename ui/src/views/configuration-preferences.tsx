import { Clock3, Download, LogIn, Play, Power, X } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import type { RunnerConfig, TimeFormat } from "@/lib/runner-api";

export function ConfigGroupHeading({
  description,
  title,
}: {
  description: string;
  title: string;
}) {
  return (
    <div className="border-b border-border pb-2">
      <h2 className="text-base font-semibold">{title}</h2>
      <p className="mt-0.5 text-xs text-muted-foreground">{description}</p>
    </div>
  );
}

export function SharedConfiguration({
  config,
  onChange,
}: {
  config: RunnerConfig;
  onChange: (config: RunnerConfig) => void;
}) {
  return (
    <div className="grid gap-4">
      <ConfigGroupHeading
        description="Preferences used by both the desktop app and the command-line interface."
        title="Shared configuration"
      />
      <div className="grid gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Date and time</CardTitle>
          </CardHeader>
          <CardContent className="p-0">
            <TimeFormatRow
              onChange={(time_format) =>
                onChange({ ...config, display: { ...config.display, time_format } })
              }
              value={config.display.time_format}
            />
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Updates</CardTitle>
          </CardHeader>
          <CardContent className="divide-y divide-border p-0">
            <SettingRow
              checked={config.updates.automatic_checks}
              description="Check the signed release feed and notify you when a new runner version is available."
              icon={Download}
              id="automatic-update-checks"
              label="Automatically check for updates"
              onChange={(automatic_checks) =>
                onChange({ ...config, updates: { ...config.updates, automatic_checks } })
              }
            />
            <label className="grid grid-cols-[minmax(0,1fr)_8rem] items-center gap-3 px-4 py-3.5 max-sm:grid-cols-1">
              <span>
                <span className="block text-sm font-medium">Check interval</span>
                <span className="mt-0.5 block text-xs leading-5 text-muted-foreground">
                  Minimum hours between repository refreshes and enabled automatic checks.
                </span>
              </span>
              <Input
                max={8_760}
                min={1}
                onChange={(event) =>
                  onChange({
                    ...config,
                    updates: {
                      ...config.updates,
                      check_interval_hours: boundedInteger(
                        event.target.valueAsNumber,
                        1,
                        8_760,
                      ),
                    },
                  })
                }
                type="number"
                value={config.updates.check_interval_hours}
              />
            </label>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

export function DesktopConfiguration({
  config,
  launchAtLoginRegistered,
  onChange,
}: {
  config: RunnerConfig;
  launchAtLoginRegistered: boolean;
  onChange: (config: RunnerConfig) => void;
}) {
  const updateDesktop = (name: keyof RunnerConfig["desktop"], checked: boolean) =>
    onChange({ ...config, desktop: { ...config.desktop, [name]: checked } });
  const registrationMatches =
    config.desktop.launch_at_login === launchAtLoginRegistered;

  return (
    <div className="grid gap-4">
      <ConfigGroupHeading
        description="Behavior that only applies to the native Windows and Linux desktop app."
        title="Desktop configuration"
      />
      <Card>
        <CardHeader className="flex flex-row flex-wrap items-center justify-between gap-3">
          <CardTitle>Startup and background behavior</CardTitle>
          <Badge
            className="max-w-full whitespace-normal text-center"
            variant={registrationMatches ? (launchAtLoginRegistered ? "good" : "muted") : "medium"}
          >
            {!registrationMatches
              ? "Save to repair launch at login"
              : launchAtLoginRegistered
                ? "Launch at login enabled"
                : "Launch at login disabled"}
          </Badge>
        </CardHeader>
        <CardContent className="divide-y divide-border p-0">
          <SettingRow
            checked={config.desktop.launch_at_login}
            description="Open BaudBound after you sign in to your Windows or Linux desktop session."
            icon={LogIn}
            id="launch-at-login"
            label="Launch at login"
            onChange={(checked) => updateDesktop("launch_at_login", checked)}
          />
          <SettingRow
            checked={config.desktop.start_background_runner_on_launch}
            description="Start trigger listeners when the desktop app opens."
            icon={Play}
            id="start-background-runner"
            label="Start background runner on launch"
            onChange={(checked) => updateDesktop("start_background_runner_on_launch", checked)}
          />
          <SettingRow
            checked={config.desktop.start_minimized_to_tray}
            description="Keep BaudBound in the system tray when it starts automatically after you sign in. Manual launches still open the window."
            disabled={!config.desktop.launch_at_login}
            icon={Power}
            id="start-minimized"
            label="Hide window when launched at login"
            onChange={(checked) => updateDesktop("start_minimized_to_tray", checked)}
          />
          <SettingRow
            checked={config.desktop.keep_running_on_close}
            description="Hide the window and keep the app in the tray when the close button is pressed."
            icon={X}
            id="keep-running-on-close"
            label="Keep running when the window closes"
            onChange={(checked) => updateDesktop("keep_running_on_close", checked)}
          />
        </CardContent>
      </Card>
    </div>
  );
}

function SettingRow({
  checked,
  description,
  disabled = false,
  icon: Icon,
  id,
  label,
  onChange,
}: {
  checked: boolean;
  description: string;
  disabled?: boolean;
  icon: typeof LogIn;
  id: string;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <div className="grid grid-cols-[minmax(0,1fr)_auto] items-start gap-3 px-4 py-3.5 max-sm:grid-cols-1">
      <div className="flex min-w-0 items-start gap-3">
        <Icon className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
        <div className="min-w-0">
          <label className="text-sm font-medium" htmlFor={id}>{label}</label>
          <p className="mt-0.5 text-xs leading-5 text-muted-foreground">{description}</p>
        </div>
      </div>
      <Switch
        checked={checked}
        className="max-sm:ml-7"
        disabled={disabled}
        id={id}
        onCheckedChange={onChange}
      />
    </div>
  );
}

function TimeFormatRow({
  onChange,
  value,
}: {
  onChange: (value: TimeFormat) => void;
  value: TimeFormat;
}) {
  return (
    <div className="grid grid-cols-[minmax(0,1fr)_auto] items-start gap-3 px-4 py-3.5 max-sm:grid-cols-1">
      <div className="flex min-w-0 items-start gap-3">
        <Clock3 className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
        <div className="min-w-0">
          <div className="text-sm font-medium">Clock format</div>
          <p className="mt-0.5 text-xs leading-5 text-muted-foreground">
            Choose how timestamps are displayed.
          </p>
        </div>
      </div>
      <div aria-label="Clock format" className="flex overflow-hidden rounded-md border border-border max-sm:ml-7 max-sm:w-fit" role="group">
        {(["12-hour", "24-hour"] as const).map((option) => (
          <Button
            aria-pressed={value === option}
            className="rounded-none border-0 first:border-r first:border-border"
            key={option}
            onClick={() => onChange(option)}
            size="sm"
            variant={value === option ? "secondary" : "outline"}
          >
            {option}
          </Button>
        ))}
      </div>
    </div>
  );
}

function boundedInteger(value: number, min: number, max: number) {
  return Number.isFinite(value)
    ? Math.min(max, Math.max(min, Math.trunc(value)))
    : min;
}

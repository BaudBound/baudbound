import { Clock3, Download, LogIn, Play, Power, Save, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import {
  type ApplicationSettingsPayload,
  type DesktopSettings,
  type TimeFormat,
  saveApplicationSettings,
} from "@/lib/runner-api";

type BooleanSetting = keyof DesktopSettings;

export function SettingsView({
  payload,
  onSaved,
}: {
  payload: ApplicationSettingsPayload;
  onSaved: (payload: ApplicationSettingsPayload) => void;
}) {
  const [draft, setDraft] = useState(payload.settings);
  const [saving, setSaving] = useState(false);

  useEffect(() => setDraft(payload.settings), [payload.settings]);

  const dirty = useMemo(
    () => JSON.stringify(draft) !== JSON.stringify(payload.settings),
    [draft, payload.settings],
  );

  function update(name: BooleanSetting, checked: boolean) {
    setDraft((current) => ({
      ...current,
      desktop: { ...current.desktop, [name]: checked },
    }));
  }

  function updateTimeFormat(timeFormat: TimeFormat) {
    setDraft((current) => ({
      ...current,
      shared: { ...current.shared, time_format: timeFormat },
    }));
  }

  async function save() {
    if (!dirty || saving) return;
    setSaving(true);
    try {
      const result = await saveApplicationSettings(draft);
      onSaved(result.payload);
      toast.success(result.message);
    } catch (error) {
      toast.error(String(error));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="grid w-full min-w-0 gap-4">
      <Card>
        <CardHeader className="flex flex-row flex-wrap items-center justify-between gap-3">
          <CardTitle>Startup and background behavior</CardTitle>
          <LoginRegistrationStatus payload={payload} />
        </CardHeader>
        <CardContent className="divide-y divide-border p-0">
          <SettingRow
            checked={draft.desktop.launch_at_login}
            description="Register BaudBound with your Windows or Linux desktop session so it opens after you sign in."
            icon={LogIn}
            label="Launch at login"
            name="launch_at_login"
            onChange={update}
          />
          <SettingRow
            checked={draft.desktop.start_background_runner_on_launch}
            description="Start hotkeys, schedules, webhooks, serial listeners, and other trigger services when the desktop app opens."
            icon={Play}
            label="Start background runner on launch"
            name="start_background_runner_on_launch"
            onChange={update}
          />
          <SettingRow
            checked={draft.desktop.start_minimized_to_tray}
            description="Keep the main window hidden in the tray when BaudBound is launched automatically after login."
            disabled={!draft.desktop.launch_at_login}
            icon={Power}
            label="Start login launch in the tray"
            name="start_minimized_to_tray"
            onChange={update}
          />
          <SettingRow
            checked={draft.desktop.keep_running_on_close}
            description="Hide the window and leave the app running in the tray when the close button is pressed."
            icon={X}
            label="Keep running when the window closes"
            name="keep_running_on_close"
            onChange={update}
          />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Date and time</CardTitle>
        </CardHeader>
        <CardContent className="p-0">
          <TimeFormatSetting value={draft.shared.time_format} onChange={updateTimeFormat} />
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Updates</CardTitle>
        </CardHeader>
        <CardContent className="p-0">
          <SettingRow
            checked={draft.desktop.automatic_update_checks}
            description="Check the signed GitHub release feed when the desktop app starts and notify you when an update is available."
            icon={Download}
            label="Automatically check for updates"
            name="automatic_update_checks"
            onChange={update}
          />
        </CardContent>
      </Card>

      <div className="flex min-w-0 justify-end max-sm:justify-stretch">
        <Button disabled={!dirty || saving} onClick={() => void save()}>
          <Save />
          {saving ? "Saving..." : "Save settings"}
        </Button>
      </div>
    </div>
  );
}

function SettingRow({
  checked,
  description,
  disabled = false,
  icon: Icon,
  label,
  name,
  onChange,
}: {
  checked: boolean;
  description: string;
  disabled?: boolean;
  icon: typeof LogIn;
  label: string;
  name: BooleanSetting;
  onChange: (name: BooleanSetting, checked: boolean) => void;
}) {
  const id = `desktop-setting-${name}`;
  return (
    <div className="grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-start gap-3 px-4 py-3.5 max-sm:grid-cols-1">
      <div className="flex min-w-0 items-start gap-3">
        <Icon className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
        <div className="min-w-0 flex-1">
          <label className="text-sm font-medium" htmlFor={id}>
            {label}
          </label>
          <p className="mt-0.5 break-words text-xs leading-5 text-muted-foreground">
            {description}
          </p>
        </div>
      </div>
      <Switch
        className="max-sm:ml-7"
        checked={checked}
        disabled={disabled}
        id={id}
        onCheckedChange={(nextChecked) => onChange(name, nextChecked)}
      />
    </div>
  );
}

function TimeFormatSetting({
  onChange,
  value,
}: {
  onChange: (value: TimeFormat) => void;
  value: TimeFormat;
}) {
  return (
    <div className="grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-start gap-3 px-4 py-3.5 max-sm:grid-cols-1">
      <div className="flex min-w-0 items-start gap-3">
        <Clock3 className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
        <div className="min-w-0">
          <div className="text-sm font-medium">Clock format</div>
          <p className="mt-0.5 break-words text-xs leading-5 text-muted-foreground">
            Choose how dates and times are displayed throughout the desktop app.
          </p>
        </div>
      </div>
      <div
        aria-label="Clock format"
        className="flex min-w-0 overflow-hidden rounded-md border border-border max-sm:ml-7 max-sm:w-fit"
        role="group"
      >
        {(["12-hour", "24-hour"] as const).map((option) => (
          <Button
            aria-pressed={value === option}
            className="rounded-none border-0 first:border-r first:border-border"
            key={option}
            onClick={() => onChange(option)}
            size="sm"
            variant={value === option ? "secondary" : "outline"}
          >
            {option === "12-hour" ? "12-hour" : "24-hour"}
          </Button>
        ))}
      </div>
    </div>
  );
}

function LoginRegistrationStatus({ payload }: { payload: ApplicationSettingsPayload }) {
  const matches = payload.settings.desktop.launch_at_login === payload.launch_at_login_registered;
  if (!matches) return <Badge variant="medium">Registration needs repair</Badge>;
  return (
    <Badge className="max-w-full whitespace-normal text-center" variant={payload.launch_at_login_registered ? "good" : "muted"}>
      {payload.launch_at_login_registered ? "Login startup registered" : "Login startup off"}
    </Badge>
  );
}

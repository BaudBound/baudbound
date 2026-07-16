import { Download, LogIn, Play, Power, Save, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import {
  type DesktopSettings,
  type DesktopSettingsPayload,
  saveDesktopSettings,
} from "@/lib/runner-api";

type BooleanSetting = keyof DesktopSettings;

export function SettingsView({
  payload,
  onSaved,
}: {
  payload: DesktopSettingsPayload;
  onSaved: (payload: DesktopSettingsPayload) => void;
}) {
  const [draft, setDraft] = useState(payload.settings);
  const [saving, setSaving] = useState(false);

  useEffect(() => setDraft(payload.settings), [payload.settings]);

  const dirty = useMemo(
    () => JSON.stringify(draft) !== JSON.stringify(payload.settings),
    [draft, payload.settings],
  );

  function update(name: BooleanSetting, checked: boolean) {
    setDraft((current) => ({ ...current, [name]: checked }));
  }

  async function save() {
    if (!dirty || saving) return;
    setSaving(true);
    try {
      const result = await saveDesktopSettings(draft);
      onSaved(result.payload);
      toast.success(result.message);
    } catch (error) {
      toast.error(String(error));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="grid max-w-4xl gap-4">
      <Card>
        <CardHeader className="flex flex-row items-center justify-between gap-3">
          <CardTitle>Startup and background behavior</CardTitle>
          <LoginRegistrationStatus payload={payload} />
        </CardHeader>
        <CardContent className="divide-y divide-border p-0">
          <SettingRow
            checked={draft.launch_at_login}
            description="Register BaudBound with your Windows or Linux desktop session so it opens after you sign in."
            icon={LogIn}
            label="Launch at login"
            name="launch_at_login"
            onChange={update}
          />
          <SettingRow
            checked={draft.start_background_runner_on_launch}
            description="Start schedules, webhooks, serial listeners, and other trigger services when the desktop app opens."
            icon={Play}
            label="Start background runner on launch"
            name="start_background_runner_on_launch"
            onChange={update}
          />
          <SettingRow
            checked={draft.start_minimized_to_tray}
            description="Keep the main window hidden in the tray when BaudBound is launched automatically after login."
            disabled={!draft.launch_at_login}
            icon={Power}
            label="Start login launch in the tray"
            name="start_minimized_to_tray"
            onChange={update}
          />
          <SettingRow
            checked={draft.keep_running_on_close}
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
          <CardTitle>Updates</CardTitle>
        </CardHeader>
        <CardContent className="p-0">
          <SettingRow
            checked={draft.automatic_update_checks}
            description="Check the signed GitHub release feed when the desktop app starts and notify you when an update is available."
            icon={Download}
            label="Automatically check for updates"
            name="automatic_update_checks"
            onChange={update}
          />
        </CardContent>
      </Card>

      <div className="flex justify-end">
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
    <div className="flex items-start gap-3 px-4 py-3.5">
      <Icon className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
      <div className="min-w-0 flex-1">
        <label className="text-sm font-medium" htmlFor={id}>
          {label}
        </label>
        <p className="mt-0.5 text-xs leading-5 text-muted-foreground">{description}</p>
      </div>
      <Switch
        checked={checked}
        disabled={disabled}
        id={id}
        onCheckedChange={(nextChecked) => onChange(name, nextChecked)}
      />
    </div>
  );
}

function LoginRegistrationStatus({ payload }: { payload: DesktopSettingsPayload }) {
  const matches = payload.settings.launch_at_login === payload.launch_at_login_registered;
  if (!matches) return <Badge variant="medium">Registration needs repair</Badge>;
  return (
    <Badge variant={payload.launch_at_login_registered ? "good" : "muted"}>
      {payload.launch_at_login_registered ? "Login startup registered" : "Login startup off"}
    </Badge>
  );
}

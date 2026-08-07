import { RotateCcw, Save } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { isValidColor } from "@/components/color-value-input";
import { DetailDialog } from "@/components/detail-dialog";
import { validateHotkey } from "@/components/hotkey-input";
import { TypedValueInput } from "@/components/typed-value-input";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { DashboardAction } from "@/lib/app-types";
import {
  type InstalledScriptSettingStatus,
  saveScriptSettings,
} from "@/lib/runner-api";
import { formatTypedValueForDisplay } from "@/lib/typed-value-display";

type SettingDraft = {
  configured: boolean;
  value: string;
};

export function ScriptSettingsDialog({
  busyActions,
  onOpenChange,
  open,
  runAction,
  scriptId,
  scriptName,
  settings,
}: {
  busyActions: Set<string>;
  onOpenChange: (open: boolean) => void;
  open: boolean;
  runAction: DashboardAction;
  scriptId: string;
  scriptName: string;
  settings: InstalledScriptSettingStatus[];
}) {
  const [drafts, setDrafts] = useState<Record<string, SettingDraft>>({});
  const saveAction = `save-script-settings:${scriptId}`;

  useEffect(() => {
    if (!open) return;
    setDrafts(
      Object.fromEntries(
        settings.map((setting) => [
          setting.name,
          initialDraft(setting),
        ]),
      ),
    );
  }, [open, settings]);

  const draftErrors = useMemo(
    () =>
      Object.fromEntries(
        settings.map((setting) => {
          const draft = drafts[setting.name] ?? initialDraft(setting);
          return [
            setting.name,
            draft.configured ? validateDraftValue(setting.value_type, setting.item_type, draft.value) : null,
          ];
        }),
      ),
    [drafts, settings],
  );
  const hasErrors = Object.values(draftErrors).some((error) => error !== null);
  const hasChanges = settings.some((setting) =>
    !draftsEqual(drafts[setting.name] ?? initialDraft(setting), initialDraft(setting)),
  );
  const missingRequired = settings.filter((setting) => {
    if (!setting.required) return false;
    const draft = drafts[setting.name] ?? initialDraft(setting);
    return !draft.configured && setting.default_value === null;
  }).length;
  const busy = busyActions.has(saveAction);

  async function save() {
    const values = settings.flatMap((setting) => {
      const draft = drafts[setting.name] ?? initialDraft(setting);
      return draft.configured ? [{ name: setting.name, value: draft.value }] : [];
    });
    const saved = await runAction(saveAction, () => saveScriptSettings(scriptId, values));
    if (saved) onOpenChange(false);
  }

  return (
    <DetailDialog
      description={`${scriptName} | ${scriptId}`}
      footer={
        <>
          <Button disabled={busy} onClick={() => onOpenChange(false)} variant="outline">
            Cancel
          </Button>
          <Button disabled={!hasChanges || hasErrors || busy || settings.length === 0} onClick={save}>
            <Save />
            Save
          </Button>
        </>
      }
      onOpenChange={onOpenChange}
      open={open}
      title="Script settings"
    >
      <div className="grid gap-4">
        <Card>
          <CardContent className="p-3 text-sm text-muted-foreground">
            Settings are stored on this runner without encryption. Use Secrets for passwords, tokens, and other
            sensitive values.
          </CardContent>
        </Card>

        {missingRequired > 0 ? (
          <div className="rounded-md border border-baud-amber/50 bg-baud-amber/10 p-3 text-sm text-baud-amber">
            {missingRequired} required setting{missingRequired === 1 ? "" : "s"} must be configured before this script
            can run.
          </div>
        ) : null}

        {settings.length === 0 ? (
          <div className="rounded-md border border-border p-4 text-sm text-muted-foreground">
            This script does not declare any settings.
          </div>
        ) : null}

        {settings.map((setting) => {
          const draft = drafts[setting.name] ?? initialDraft(setting);
          const draftError = draftErrors[setting.name];
          return (
            <Card key={setting.name}>
              <CardHeader className="flex-row items-start justify-between gap-3">
                <div className="grid gap-1">
                  <CardTitle>{setting.name}</CardTitle>
                  {setting.description ? <p className="text-sm text-muted-foreground">{setting.description}</p> : null}
                </div>
                <div className="flex flex-wrap justify-end gap-1.5">
                  <Badge variant="muted">{setting.value_type}</Badge>
                  <Badge
                    variant={setting.required && !draft.configured && setting.default_value === null ? "medium" : "muted"}
                  >
                    {setting.required ? "Required" : "Optional"}
                  </Badge>
                  <Badge variant={draft.configured ? "good" : "muted"}>
                    {draft.configured ? "Configured" : "Package value"}
                  </Badge>
                  <Button
                    aria-label={`Reset ${setting.name} to its package value`}
                    className="size-7 shrink-0 p-0"
                    disabled={!draft.configured || busy}
                    onClick={() =>
                      setDrafts((current) => ({
                        ...current,
                        [setting.name]: packageValueDraft(setting),
                      }))
                    }
                    size="sm"
                    title={`Reset ${setting.name} to its package value`}
                    variant="outline"
                  >
                    <RotateCcw />
                  </Button>
                </div>
              </CardHeader>
              <CardContent className="grid gap-3">
                <div className="grid gap-1.5">
                  <label className="text-xs text-muted-foreground" htmlFor={settingInputId(scriptId, setting.name)}>
                    Value
                  </label>
                  <TypedValueInput
                    id={settingInputId(scriptId, setting.name)}
                    itemType={setting.item_type}
                    onChange={(value) =>
                      setDrafts((current) => ({ ...current, [setting.name]: { configured: true, value } }))
                    }
                    value={draft.value}
                    valueType={setting.value_type}
                  />
                  {draftError ? <p className="text-xs text-destructive">{draftError}</p> : null}
                </div>
                <div className="grid gap-2 md:grid-cols-2">
                  <ValuePreview
                    itemType={setting.item_type}
                    label="Package default"
                    value={setting.default_value}
                    valueType={setting.value_type}
                  />
                  <ValuePreview
                    itemType={setting.item_type}
                    label="Value after saving"
                    value={draftValueForPreview(setting, draft)}
                    valueType={setting.value_type}
                  />
                </div>
              </CardContent>
            </Card>
          );
        })}
      </div>
    </DetailDialog>
  );
}

function initialDraft(setting: InstalledScriptSettingStatus): SettingDraft {
  return {
    configured: setting.configured,
    value: editableValue(setting.configured_value ?? setting.default_value, setting.value_type),
  };
}

function packageValueDraft(setting: InstalledScriptSettingStatus): SettingDraft {
  return {
    configured: false,
    value: editableValue(setting.default_value, setting.value_type),
  };
}

function draftsEqual(left: SettingDraft, right: SettingDraft) {
  return left.configured === right.configured && left.value === right.value;
}

function draftValueForPreview(setting: InstalledScriptSettingStatus, draft: SettingDraft): unknown {
  if (!draft.configured) return setting.default_value;
  if (
    setting.value_type === "string" ||
    setting.value_type === "keyboard_key" ||
    setting.value_type === "color"
  ) {
    return draft.value;
  }
  if (setting.value_type === "integer") return Math.trunc(Number(draft.value));
  if (setting.value_type === "float") return Number(draft.value);
  if (setting.value_type === "boolean") {
    if (draft.value === "true") return true;
    if (draft.value === "false") return false;
    return undefined;
  }
  try {
    return JSON.parse(draft.value) as unknown;
  } catch {
    return undefined;
  }
}

function ValuePreview({
  itemType,
  label,
  value,
  valueType,
}: {
  itemType: InstalledScriptSettingStatus["item_type"];
  label: string;
  value: unknown | null;
  valueType: InstalledScriptSettingStatus["value_type"];
}) {
  return (
    <div className="grid gap-1.5">
      <div className="text-xs text-muted-foreground">{label}</div>
      <pre className="min-h-10 max-w-full select-text overflow-x-hidden whitespace-pre-wrap break-all rounded-md border border-border bg-background p-2 text-xs">
        {value === null ? "Not set" : formatTypedValueForDisplay(valueType, value, itemType)}
      </pre>
    </div>
  );
}

function editableValue(value: unknown | null, valueType: InstalledScriptSettingStatus["value_type"]) {
  if (value === null || value === undefined) return "";
  if (valueType === "object" || valueType === "list" || valueType === "datetime" || valueType === "duration") {
    return JSON.stringify(value, null, 2);
  }
  return String(value);
}

export function validateDraftValue(
  valueType: InstalledScriptSettingStatus["value_type"],
  itemType: InstalledScriptSettingStatus["item_type"],
  value: string,
): string | null {
  if (valueType === "string") return null;
  if (valueType === "keyboard_key") {
    return validateHotkey(value) ? null : "Press a valid Windows key combination.";
  }
  if (valueType === "color") {
    return isValidColor(value) ? null : "Select a color in #RRGGBB format.";
  }
  if (valueType === "integer") {
    const parsed = Number(value);
    return value.trim() && Number.isInteger(parsed) ? null : "Enter a whole number.";
  }
  if (valueType === "float") {
    const parsed = Number(value);
    return value.trim() && Number.isFinite(parsed) ? null : "Enter a finite number.";
  }
  if (valueType === "boolean") {
    return value === "true" || value === "false" ? null : "Select true or false.";
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(value);
  } catch {
    return "Enter valid JSON.";
  }
  if (valueType === "object") {
    return isRecord(parsed) ? null : "Enter a JSON object.";
  }
  if (valueType === "list") {
    if (!Array.isArray(parsed)) return "Enter a list.";
    if (!itemType) return "This package does not declare a list item type.";
    for (const [index, item] of parsed.entries()) {
      if (!valueMatchesType(itemType, item)) {
        return `List item ${index + 1} does not match type ${itemType}.`;
      }
    }
    return null;
  }
  return valueMatchesType(valueType, parsed)
    ? null
    : valueType === "datetime"
      ? "Select a valid date and time."
      : "Enter a nonnegative duration and select its unit.";
}

function valueMatchesType(
  valueType: NonNullable<InstalledScriptSettingStatus["item_type"]> | "datetime" | "duration",
  value: unknown,
) {
  if (valueType === "string") return typeof value === "string";
  if (valueType === "keyboard_key") return typeof value === "string" && validateHotkey(value);
  if (valueType === "color") return typeof value === "string" && isValidColor(value);
  if (valueType === "integer") return typeof value === "number" && Number.isInteger(value);
  if (valueType === "float") return typeof value === "number" && Number.isFinite(value);
  if (valueType === "boolean") return typeof value === "boolean";
  if (valueType === "object") return isRecord(value);
  if (!isRecord(value)) return false;
  if (valueType === "datetime") {
    return value.type === "datetime" && typeof value.value === "string" && !Number.isNaN(Date.parse(value.value));
  }
  return (
    value.type === "duration" &&
    typeof value.value === "number" &&
    Number.isFinite(value.value) &&
    value.value >= 0 &&
    typeof value.unit === "string" &&
    ["milliseconds", "seconds", "minutes", "hours", "days"].includes(value.unit)
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function settingInputId(scriptId: string, name: string) {
  return `script-setting-${scriptId}-${name}`;
}

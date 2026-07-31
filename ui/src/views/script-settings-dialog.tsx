import { RotateCcw, Save } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { DetailDialog } from "@/components/detail-dialog";
import { isValidColor } from "@/components/color-value-input";
import { validateHotkey } from "@/components/hotkey-input";
import { TypedValueInput } from "@/components/typed-value-input";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { DashboardAction } from "@/lib/app-types";
import {
  type InstalledScriptSettingStatus,
  resetScriptSettings,
  setScriptSetting,
  unsetScriptSetting,
} from "@/lib/runner-api";

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
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const configuredCount = settings.filter((setting) => setting.configured).length;
  const resetAllAction = `reset-script-settings:${scriptId}`;

  useEffect(() => {
    if (!open) return;
    setDrafts(
      Object.fromEntries(
        settings.map((setting) => [
          setting.name,
          editableValue(setting.configured_value ?? setting.default_value, setting.value_type),
        ]),
      ),
    );
  }, [open, settings]);

  const missingRequired = useMemo(
    () =>
      settings.filter(
        (setting) => setting.required && setting.effective_value === null,
      ).length,
    [settings],
  );

  return (
    <DetailDialog
      description={`${scriptName} | ${scriptId}`}
      onOpenChange={onOpenChange}
      open={open}
      title="Script Settings"
    >
      <div className="grid gap-4">
        <Card>
          <CardContent className="flex flex-wrap items-center justify-between gap-3 p-3">
            <div className="text-sm text-muted-foreground">
              Settings are stored on this runner without encryption. Use Secrets for passwords,
              tokens, and other sensitive values.
            </div>
            <Button
              disabled={configuredCount === 0 || busyActions.has(resetAllAction)}
              onClick={() => runAction(resetAllAction, () => resetScriptSettings(scriptId))}
              size="sm"
              variant="outline"
            >
              <RotateCcw />
              Reset all
            </Button>
          </CardContent>
        </Card>

        {missingRequired > 0 ? (
          <div className="rounded-md border border-baud-amber/50 bg-baud-amber/10 p-3 text-sm text-baud-amber">
            {missingRequired} required setting{missingRequired === 1 ? "" : "s"} must be configured
            before this script can run.
          </div>
        ) : null}

        {settings.map((setting) => {
          const saveAction = `set-script-setting:${scriptId}:${setting.name}`;
          const resetAction = `unset-script-setting:${scriptId}:${setting.name}`;
          const draft = drafts[setting.name] ?? "";
          const draftError = validateDraftValue(
            setting.value_type,
            setting.item_type,
            draft,
          );
          return (
            <Card key={setting.name}>
              <CardHeader className="flex-row items-start justify-between gap-3">
                <div className="grid gap-1">
                  <CardTitle>{setting.name}</CardTitle>
                  {setting.description ? (
                    <p className="text-sm text-muted-foreground">{setting.description}</p>
                  ) : null}
                </div>
                <div className="flex flex-wrap justify-end gap-1.5">
                  <Badge variant="muted">{setting.value_type}</Badge>
                  <Badge
                    variant={
                      setting.required && setting.effective_value === null
                        ? "medium"
                        : "muted"
                    }
                  >
                    {setting.required ? "Required" : "Optional"}
                  </Badge>
                  <Badge variant={setting.configured ? "good" : "muted"}>
                    {setting.configured ? "Overridden" : "Package value"}
                  </Badge>
                </div>
              </CardHeader>
              <CardContent className="grid gap-3">
                <div className="grid gap-1.5">
                  <label className="text-xs text-muted-foreground" htmlFor={settingInputId(scriptId, setting.name)}>
                    Configured override
                  </label>
                  <TypedValueInput
                    id={settingInputId(scriptId, setting.name)}
                    itemType={setting.item_type}
                    onChange={(value) =>
                      setDrafts((current) => ({ ...current, [setting.name]: value }))
                    }
                    value={draft}
                    valueType={setting.value_type}
                  />
                  {draftError ? (
                    <p className="text-xs text-destructive">{draftError}</p>
                  ) : null}
                </div>
                <div className="grid gap-2 md:grid-cols-2">
                  <ValuePreview label="Package default" value={setting.default_value} />
                  <ValuePreview label="Effective value" value={setting.effective_value} />
                </div>
                <div className="flex flex-wrap justify-end gap-2">
                  <Button
                    disabled={!setting.configured || busyActions.has(resetAction)}
                    onClick={() =>
                      runAction(resetAction, () =>
                        unsetScriptSetting(scriptId, setting.name),
                      )
                    }
                    size="sm"
                    variant="outline"
                  >
                    <RotateCcw />
                    Reset to package value
                  </Button>
                  <Button
                    disabled={
                      draftError !== null ||
                      busyActions.has(saveAction)
                    }
                    onClick={() =>
                      runAction(saveAction, () =>
                        setScriptSetting(scriptId, setting.name, draft),
                      )
                    }
                    size="sm"
                  >
                    <Save />
                    Save override
                  </Button>
                </div>
              </CardContent>
            </Card>
          );
        })}
      </div>
    </DetailDialog>
  );
}

function ValuePreview({ label, value }: { label: string; value: unknown | null }) {
  return (
    <div className="grid gap-1.5">
      <div className="text-xs text-muted-foreground">{label}</div>
      <pre className="min-h-10 select-text overflow-x-auto rounded-md border border-border bg-background p-2 text-xs">
        {value === null ? "Not set" : displayValue(value)}
      </pre>
    </div>
  );
}

function editableValue(
  value: unknown | null,
  valueType: InstalledScriptSettingStatus["value_type"],
) {
  if (value === null || value === undefined) return "";
  if (
    valueType === "object" ||
    valueType === "list" ||
    valueType === "datetime" ||
    valueType === "duration"
  ) {
    return JSON.stringify(value, null, 2);
  }
  return String(value);
}

function displayValue(value: unknown) {
  return typeof value === "string" ? value : JSON.stringify(value, null, 2);
}

function validateDraftValue(
  valueType: InstalledScriptSettingStatus["value_type"],
  itemType: InstalledScriptSettingStatus["item_type"],
  value: string,
): string | null {
  if (valueType === "string") return null;
  if (valueType === "file_path") {
    return value.trim() ? null : "Enter a file path.";
  }
  if (valueType === "hotkey") {
    return validateHotkey(value) ? null : "Press a valid Windows key combination.";
  }
  if (valueType === "color") {
    return isValidColor(value) ? null : "Select a color in #RRGGBB format.";
  }
  if (valueType === "number") {
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
  if (valueType === "file_path") return typeof value === "string" && value.trim().length > 0;
  if (valueType === "number") return typeof value === "number" && Number.isFinite(value);
  if (valueType === "boolean") return typeof value === "boolean";
  if (valueType === "object") return isRecord(value);
  if (!isRecord(value)) return false;
  if (valueType === "datetime") {
    return value.type === "datetime" &&
      typeof value.value === "string" &&
      !Number.isNaN(Date.parse(value.value));
  }
  return value.type === "duration" &&
    typeof value.value === "number" &&
    Number.isFinite(value.value) &&
    value.value >= 0 &&
    typeof value.unit === "string" &&
    ["milliseconds", "seconds", "minutes", "hours", "days"].includes(value.unit);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function settingInputId(scriptId: string, name: string) {
  return `script-setting-${scriptId}-${name}`;
}

import type { InstalledScriptSettingStatus } from "@/lib/runner-api";

type ValueType = InstalledScriptSettingStatus["value_type"];
type ItemType = NonNullable<InstalledScriptSettingStatus["item_type"]>;
type DisplayType = ValueType | ItemType;

const durationUnits = ["milliseconds", "seconds", "minutes", "hours", "days"] as const;
type DurationUnit = (typeof durationUnits)[number];

export function formatTypedValueForDisplay(
  valueType: ValueType,
  value: unknown,
  itemType: InstalledScriptSettingStatus["item_type"] = null,
): string {
  if (valueType === "list") {
    if (!Array.isArray(value)) return "Invalid list";
    if (value.length === 0) return "No items";
    return value
      .map((item) => formatValue(itemType ?? inferValueType(item) ?? "object", item))
      .join("\n");
  }
  return formatValue(valueType, value);
}

function formatValue(valueType: DisplayType, value: unknown): string {
  if (valueType === "string" || valueType === "keyboard_key" || valueType === "color") {
    return typeof value === "string" ? value : "Invalid value";
  }
  if (valueType === "integer") {
    return typeof value === "number" && Number.isInteger(value) ? String(value) : "Invalid integer";
  }
  if (valueType === "float") {
    return typeof value === "number" && Number.isFinite(value) ? String(value) : "Invalid float";
  }
  if (valueType === "boolean") {
    return typeof value === "boolean" ? String(value) : "Invalid boolean";
  }
  if (valueType === "datetime") {
    return typedDatetimeValue(value) ?? "Invalid date and time";
  }
  if (valueType === "duration") {
    if (!isRecord(value) || value.type !== "duration" || typeof value.value !== "number" || !Number.isFinite(value.value)) {
      return "Invalid duration";
    }
    if (value.value < 0 || typeof value.unit !== "string" || !durationUnits.includes(value.unit as DurationUnit)) {
      return "Invalid duration";
    }
    const unit = durationUnitLabel(value.unit as DurationUnit, value.value);
    return `${value.value} ${unit}`;
  }
  return isRecord(value) ? JSON.stringify(value, null, 2) : "Invalid object";
}

function typedDatetimeValue(value: unknown): string | null {
  if (!isRecord(value) || value.type !== "datetime" || typeof value.value !== "string") return null;
  return Number.isFinite(Date.parse(value.value)) ? value.value : null;
}

function inferValueType(value: unknown): ItemType | null {
  if (typeof value === "string") return "string";
  // An integer and a float are separate types, so the inferred type follows
  // the value rather than collapsing both into one numeric type.
  if (typeof value === "number" && Number.isInteger(value)) return "integer";
  if (typeof value === "number" && Number.isFinite(value)) return "float";
  if (typeof value === "boolean") return "boolean";
  if (!isRecord(value)) return null;
  if (value.type === "datetime" && typeof value.value === "string") return "datetime";
  if (value.type === "duration" && typeof value.value === "number") return "duration";
  return "object";
}

function durationUnitLabel(unit: DurationUnit, amount: number) {
  return amount === 1 ? unit.replace(/s$/, "") : unit;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

import CodeMirror from "@uiw/react-codemirror";
import { ArrowDown, ArrowUp, Plus, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { ColorValueInput } from "@/components/color-value-input";
import { HotkeyInput } from "@/components/hotkey-input";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import {
  runtimeFloatMaximum,
  runtimeFloatMinimum,
  type NumericFieldContract,
} from "@/components/numeric-field-model";
import { NumericField } from "@/components/numeric-field";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { InstalledScriptSettingStatus } from "@/lib/runner-api";

type ValueType = InstalledScriptSettingStatus["value_type"];
type ItemType = NonNullable<InstalledScriptSettingStatus["item_type"]>;

const durationUnits = ["milliseconds", "seconds", "minutes", "hours", "days"] as const;
const runtimeNumberContract: NumericFieldContract = {
  kind: "float",
  maximum: runtimeFloatMaximum,
  minimum: runtimeFloatMinimum,
  signed: true,
};
const durationAmountContract: NumericFieldContract = {
  ...runtimeNumberContract,
  minimum: "0",
  signed: false,
};

export function TypedValueInput({
  id,
  itemType,
  onChange,
  value,
  valueType,
}: {
  id: string;
  itemType: InstalledScriptSettingStatus["item_type"];
  onChange: (value: string) => void;
  value: string;
  valueType: ValueType;
}) {
  if (valueType === "boolean") {
    return (
      <Select onValueChange={onChange} value={value || "false"}>
        <SelectTrigger id={id}>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="true">True</SelectItem>
          <SelectItem value="false">False</SelectItem>
        </SelectContent>
      </Select>
    );
  }

  if (valueType === "number") {
    return (
      <NumericField
        ariaLabel="Number value"
        contract={runtimeNumberContract}
        id={id}
        onChange={onChange}
        value={value}
      />
    );
  }

  if (valueType === "object") {
    return (
      <div className="overflow-hidden rounded-md border border-border">
        <CodeMirror
          basicSetup={{ foldGutter: true, lineNumbers: true }}
          height="160px"
          id={id}
          onChange={onChange}
          theme="dark"
          value={value}
        />
      </div>
    );
  }

  if (valueType === "list") {
    return (
      <ListInput
        id={id}
        itemType={itemType ?? "string"}
        onChange={onChange}
        value={value}
      />
    );
  }

  if (valueType === "datetime") {
    const parsed = parseRecord(value);
    const iso = typeof parsed?.value === "string" ? parsed.value : new Date(0).toISOString();
    return (
      <div className="grid gap-1.5">
        <Input
          id={id}
          onChange={(event) => {
            const date = new Date(event.target.value);
            if (!Number.isNaN(date.getTime())) {
              onChange(JSON.stringify({ type: "datetime", value: date.toISOString() }));
            }
          }}
          step="1"
          type="datetime-local"
          value={toDatetimeLocalValue(iso)}
        />
        <p className="text-xs text-muted-foreground">
          Entered in your local time zone and stored as RFC 3339.
        </p>
      </div>
    );
  }

  if (valueType === "duration") {
    const parsed = parseRecord(value);
    const amount = typeof parsed?.value === "number" ? parsed.value : 0;
    const unit = durationUnits.includes(parsed?.unit as (typeof durationUnits)[number])
      ? String(parsed?.unit)
      : "seconds";
    return (
      <div className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_11rem]">
        <NumericField
          ariaLabel="Duration amount"
          contract={durationAmountContract}
          id={id}
          onChange={(nextDraft) => {
            const next = Number(nextDraft);
            if (nextDraft.trim() && Number.isFinite(next) && next >= 0) {
              onChange(JSON.stringify({ type: "duration", unit, value: next }));
            }
          }}
          value={amount}
        />
        <Select
          onValueChange={(next) =>
            onChange(JSON.stringify({ type: "duration", unit: next, value: amount }))
          }
          value={unit}
        >
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {durationUnits.map((entry) => (
              <SelectItem key={entry} value={entry}>
                {entry}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
    );
  }

  if (valueType === "hotkey") {
    return <HotkeyInput id={id} onChange={onChange} value={value} />;
  }

  if (valueType === "color") {
    return <ColorValueInput id={id} onChange={onChange} value={value} />;
  }

  if (valueType === "string") {
    return (
      <Textarea
        className="min-h-24 w-full resize-y"
        id={id}
        onChange={(event) => onChange(event.target.value)}
        value={value}
      />
    );
  }

  return (
    <Input
      id={id}
      onChange={(event) => onChange(event.target.value)}
      type="text"
      value={value}
    />
  );
}

function ListInput({
  id,
  itemType,
  onChange,
  value,
}: {
  id: string;
  itemType: ItemType;
  onChange: (value: string) => void;
  value: string;
}) {
  const items = parseList(value);
  return (
    <div className="grid gap-2">
      <div className="text-xs text-muted-foreground">
        Every item uses type <span className="font-mono text-foreground">{itemType}</span>.
      </div>
      {items.map((item, index) => (
        <div className="grid grid-cols-[minmax(0,1fr)_auto] items-start gap-2" key={index}>
          <TypedValueInput
            id={`${id}-${index}`}
            itemType={null}
            onChange={(next) => {
              const updated = [...items];
              updated[index] = parseItemValue(itemType, next);
              onChange(JSON.stringify(updated));
            }}
            value={editableItemValue(itemType, item)}
            valueType={itemType}
          />
          <div className="flex gap-1">
            <Button
              aria-label={`Move item ${index + 1} up`}
              className="size-9 shrink-0 p-0"
              disabled={index === 0}
              onClick={() => onChange(JSON.stringify(moveItem(items, index, index - 1)))}
              size="sm"
              type="button"
              variant="outline"
            >
              <ArrowUp />
            </Button>
            <Button
              aria-label={`Move item ${index + 1} down`}
              className="size-9 shrink-0 p-0"
              disabled={index === items.length - 1}
              onClick={() => onChange(JSON.stringify(moveItem(items, index, index + 1)))}
              size="sm"
              type="button"
              variant="outline"
            >
              <ArrowDown />
            </Button>
            <Button
              aria-label={`Remove item ${index + 1}`}
              className="size-9 shrink-0 p-0"
              onClick={() => onChange(JSON.stringify(items.filter((_, itemIndex) => itemIndex !== index)))}
              size="sm"
              type="button"
              variant="outline"
            >
              <Trash2 />
            </Button>
          </div>
        </div>
      ))}
      <Button
        className="justify-self-start"
        onClick={() => onChange(JSON.stringify([...items, emptyItemValue(itemType)]))}
        size="sm"
        type="button"
        variant="outline"
      >
        <Plus />
        Add item
      </Button>
    </div>
  );
}

function moveItem(items: unknown[], from: number, to: number) {
  const next = [...items];
  const [item] = next.splice(from, 1);
  if (item !== undefined) next.splice(to, 0, item);
  return next;
}

function parseList(value: string): unknown[] {
  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function parseRecord(value: string): Record<string, unknown> | null {
  try {
    const parsed = JSON.parse(value);
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

function editableItemValue(type: ItemType, value: unknown) {
  return type === "object" || type === "datetime" || type === "duration"
    ? JSON.stringify(value, null, 2)
    : String(value ?? "");
}

function parseItemValue(type: ItemType, value: string): unknown {
  if (type === "number") return Number(value);
  if (type === "boolean") return value === "true";
  if (type === "object" || type === "datetime" || type === "duration") {
    try {
      return JSON.parse(value);
    } catch {
      return emptyItemValue(type);
    }
  }
  return value;
}

function emptyItemValue(type: ItemType): unknown {
  if (type === "number") return 0;
  if (type === "boolean") return false;
  if (type === "object") return {};
  if (type === "datetime") return { type: "datetime", value: new Date(0).toISOString() };
  if (type === "duration") return { type: "duration", unit: "seconds", value: 0 };
  return "";
}

function toDatetimeLocalValue(iso: string) {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  const localTime = new Date(date.getTime() - date.getTimezoneOffset() * 60_000);
  return localTime.toISOString().slice(0, 19);
}

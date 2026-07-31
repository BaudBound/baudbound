import { Input } from "@/components/ui/input";

const COLOR_PATTERN = /^#[0-9a-f]{6}$/i;

export function ColorValueInput({
  id,
  onChange,
  value,
}: {
  id: string;
  onChange: (value: string) => void;
  value: string;
}) {
  const pickerColor = COLOR_PATTERN.test(value) ? value : "#000000";

  return (
    <div className="flex min-h-9 overflow-hidden rounded-md border border-border bg-[#080b12] focus-within:border-ring focus-within:ring-2 focus-within:ring-ring/25">
      <label
        className="relative block w-11 shrink-0 cursor-pointer border-r border-border"
        style={{ backgroundColor: pickerColor }}
      >
        <span className="sr-only">Choose color</span>
        <input
          aria-label="Choose color"
          className="absolute inset-0 cursor-pointer opacity-0"
          onChange={(event) => onChange(event.target.value.toUpperCase())}
          type="color"
          value={pickerColor}
        />
      </label>
      <Input
        className="min-w-0 flex-1 rounded-none border-0 bg-transparent focus:border-0 focus:ring-0"
        id={id}
        onChange={(event) => onChange(event.target.value)}
        placeholder="#RRGGBB"
        value={value}
      />
    </div>
  );
}

export function isValidColor(value: string) {
  return COLOR_PATTERN.test(value);
}

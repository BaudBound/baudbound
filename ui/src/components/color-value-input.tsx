import { useCallback, useEffect, useRef, useState } from "react";

import { ColorPicker, ColorPickerHue, ColorPickerSelection } from "@/components/ui/color-picker";
import { Input } from "@/components/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { cn } from "@/lib/utils";

const COLOR_PATTERN = /^#[0-9a-f]{6}$/i;
const FALLBACK_COLOR = "#000000";
const INVALID_COLOR_BACKGROUND =
  "repeating-conic-gradient(rgb(114 125 149 / 45%) 0 25%, rgb(23 27 39) 0 50%) 50% / 8px 8px";

export function ColorValueInput({
  ariaDescribedBy,
  ariaLabelledBy,
  className,
  id,
  invalid = false,
  label = "Color",
  onChange,
  value,
}: {
  ariaDescribedBy?: string;
  ariaLabelledBy?: string;
  className?: string;
  id: string;
  invalid?: boolean;
  label?: string;
  onChange: (value: string) => void;
  value: string;
}) {
  const currentColor = isValidColor(value) ? value.toUpperCase() : undefined;
  const [open, setOpen] = useState(false);
  const [pickerStartColor, setPickerStartColor] = useState(currentColor ?? FALLBACK_COLOR);
  const hasInteracted = useRef(false);
  const onChangeRef = useRef(onChange);

  useEffect(() => {
    onChangeRef.current = onChange;
  }, [onChange]);

  const handlePickerChange = useCallback((rgba: [number, number, number, number]) => {
    if (hasInteracted.current) onChangeRef.current(rgbaToHex(rgba));
  }, []);

  function handleOpenChange(nextOpen: boolean) {
    if (nextOpen) {
      hasInteracted.current = false;
      setPickerStartColor(currentColor ?? FALLBACK_COLOR);
    }
    setOpen(nextOpen);
  }

  return (
    <div
      className={cn(
        "flex h-9 min-w-0 overflow-visible rounded-md border bg-[#080b12] transition-colors focus-within:border-ring focus-within:ring-2 focus-within:ring-ring/25",
        invalid ? "border-baud-danger" : "border-border",
        className,
      )}
    >
      <Popover onOpenChange={handleOpenChange} open={open}>
        <PopoverTrigger asChild>
          <button
            aria-expanded={open}
            aria-label={`Open ${label.toLowerCase()} color picker`}
            className="h-full w-11 shrink-0 rounded-l-[5px] border-0 border-r border-border outline-none transition-[filter] hover:brightness-110 focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/45"
            style={{ background: currentColor ?? INVALID_COLOR_BACKGROUND }}
            type="button"
          />
        </PopoverTrigger>
        <PopoverContent
          align="start"
          collisionPadding={12}
          side="bottom"
          sideOffset={6}
        >
          <div className="mb-2 text-sm font-medium">{label}</div>
          <div
            onKeyDownCapture={() => {
              hasInteracted.current = true;
            }}
            onPointerDownCapture={() => {
              hasInteracted.current = true;
            }}
          >
            <ColorPicker
              className="w-full"
              defaultValue={pickerStartColor}
              key={pickerStartColor}
              onChange={handlePickerChange}
            >
              <ColorPickerSelection
                aria-label={`${label} saturation and lightness`}
                className="h-40"
              />
              <ColorPickerHue aria-label={`${label} hue`} />
            </ColorPicker>
          </div>
        </PopoverContent>
      </Popover>
      <Input
        aria-describedby={ariaDescribedBy}
        aria-invalid={invalid || undefined}
        aria-labelledby={ariaLabelledBy}
        className="h-full min-h-0 min-w-0 flex-1 rounded-none border-0 bg-transparent font-mono focus:border-0 focus:ring-0"
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

export function rgbaToHex(rgba: [number, number, number, number]) {
  return `#${rgba
    .slice(0, 3)
    .map((channel) => Math.round(channel).toString(16).padStart(2, "0"))
    .join("")
    .toUpperCase()}`;
}

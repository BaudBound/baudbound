import Color from "color";
import { Slider } from "radix-ui";
import {
  type ComponentProps,
  createContext,
  type HTMLAttributes,
  memo,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { cn } from "@/lib/utils";

type ColorPickerContextValue = {
  hue: number;
  lightness: number;
  saturation: number;
  setHue: (hue: number) => void;
  setLightness: (lightness: number) => void;
  setSaturation: (saturation: number) => void;
};

const ColorPickerContext = createContext<ColorPickerContextValue | undefined>(
  undefined,
);

function useColorPicker() {
  const context = useContext(ColorPickerContext);
  if (!context) {
    throw new Error("Color picker controls must be rendered inside ColorPicker.");
  }
  return context;
}

export type ColorPickerProps = Omit<
  HTMLAttributes<HTMLDivElement>,
  "defaultValue" | "onChange"
> & {
  defaultValue?: Parameters<typeof Color>[0];
  onChange?: (value: [number, number, number, number]) => void;
  value?: Parameters<typeof Color>[0];
};

export function ColorPicker({
  className,
  defaultValue = "#000000",
  onChange,
  value,
  ...props
}: ColorPickerProps) {
  const initialColor = resolveColor(value, defaultValue);
  const [hue, setHue] = useState(initialColor.hue());
  const [saturation, setSaturation] = useState(initialColor.saturationl());
  const [lightness, setLightness] = useState(initialColor.lightness());

  useEffect(() => {
    if (value === undefined) return;
    const color = resolveColor(value, defaultValue);
    setHue(color.hue());
    setSaturation(color.saturationl());
    setLightness(color.lightness());
  }, [defaultValue, value]);

  useEffect(() => {
    if (!onChange) return;
    const rgba = Color.hsl(hue, saturation, lightness).rgb().array();
    onChange([Number(rgba[0]), Number(rgba[1]), Number(rgba[2]), 1]);
  }, [hue, saturation, lightness, onChange]);

  return (
    <ColorPickerContext.Provider
      value={{
        hue,
        lightness,
        saturation,
        setHue,
        setLightness,
        setSaturation,
      }}
    >
      <div className={cn("flex flex-col gap-3", className)} {...props} />
    </ColorPickerContext.Provider>
  );
}

export type ColorPickerSelectionProps = HTMLAttributes<HTMLDivElement>;

export const ColorPickerSelection = memo(function ColorPickerSelection({
  className,
  ...props
}: ColorPickerSelectionProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [isDragging, setIsDragging] = useState(false);
  const { hue, saturation, lightness, setSaturation, setLightness } =
    useColorPicker();
  const positionX = saturation / 100;
  const topLightness = positionX < 0.01 ? 100 : 50 + 50 * (1 - positionX);
  const positionY = Math.max(0, Math.min(1, 1 - lightness / topLightness));
  const background = useMemo(
    () =>
      `linear-gradient(0deg, rgb(0 0 0), transparent), linear-gradient(90deg, rgb(255 255 255), transparent), hsl(${hue} 100% 50%)`,
    [hue],
  );

  const updateSelection = useCallback(
    (event: PointerEvent) => {
      const rect = containerRef.current?.getBoundingClientRect();
      if (!rect) return;
      const x = Math.max(0, Math.min(1, (event.clientX - rect.left) / rect.width));
      const y = Math.max(0, Math.min(1, (event.clientY - rect.top) / rect.height));
      const selectedTopLightness = x < 0.01 ? 100 : 50 + 50 * (1 - x);
      setSaturation(x * 100);
      setLightness(selectedTopLightness * (1 - y));
    },
    [setLightness, setSaturation],
  );

  useEffect(() => {
    if (!isDragging) return;
    const handlePointerMove = (event: PointerEvent) => updateSelection(event);
    const handlePointerUp = () => setIsDragging(false);
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);
    return () => {
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
    };
  }, [isDragging, updateSelection]);

  return (
    <div
      className={cn("relative cursor-crosshair touch-none rounded", className)}
      onPointerDown={(event) => {
        event.preventDefault();
        updateSelection(event.nativeEvent);
        setIsDragging(true);
      }}
      ref={containerRef}
      style={{ background }}
      {...props}
    >
      <div
        className="pointer-events-none absolute size-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-[0_0_0_1px_rgb(0_0_0/0.5)]"
        style={{ left: `${positionX * 100}%`, top: `${positionY * 100}%` }}
      />
    </div>
  );
});

export type ColorPickerHueProps = ComponentProps<typeof Slider.Root>;

export function ColorPickerHue({ className, ...props }: ColorPickerHueProps) {
  const { hue, setHue } = useColorPicker();
  return (
    <Slider.Root
      className={cn("relative flex h-4 w-full touch-none", className)}
      max={360}
      onValueChange={([nextHue]) => setHue(nextHue)}
      step={1}
      value={[hue]}
      {...props}
    >
      <Slider.Track className="relative my-0.5 h-3 w-full grow rounded-full bg-[linear-gradient(90deg,#FF0000,#FFFF00,#00FF00,#00FFFF,#0000FF,#FF00FF,#FF0000)]">
        <Slider.Range className="absolute h-full" />
      </Slider.Track>
      <Slider.Thumb className="block size-4 rounded-full border border-primary/50 bg-background shadow outline-none focus-visible:ring-2 focus-visible:ring-ring/45" />
    </Slider.Root>
  );
}

function resolveColor(
  value: Parameters<typeof Color>[0] | undefined,
  fallback: Parameters<typeof Color>[0],
) {
  try {
    return Color(value ?? fallback);
  } catch {
    return Color(fallback);
  }
}

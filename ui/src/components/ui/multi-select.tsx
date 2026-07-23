import { CheckIcon, ChevronDownIcon, XIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

export type MultiSelectOption = {
  label: string;
  value: string;
};

export function MultiSelect({
  className,
  options,
  placeholder = "Select options",
  value,
  onChange,
}: {
  className?: string;
  options: MultiSelectOption[];
  placeholder?: string;
  value: string[];
  onChange: (value: string[]) => void;
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    function onPointerDown(event: PointerEvent) {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    }

    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, []);

  function toggleOption(option: string) {
    if (value.includes(option)) {
      onChange(value.filter((item) => item !== option));
      return;
    }
    onChange([...value, option]);
  }

  return (
    <div
      className={cn("relative", className)}
      data-slot="multi-select"
      ref={rootRef}
    >
      <Button
        aria-expanded={open}
        className="h-auto min-h-9 w-full justify-between whitespace-normal px-2 py-1.5"
        onClick={() => setOpen((current) => !current)}
        variant="outline"
      >
        <span className="flex min-w-0 flex-1 flex-wrap gap-1.5 text-left">
          {value.length === 0 ? (
            <span className="px-1 text-muted-foreground">{placeholder}</span>
          ) : (
            value.map((item) => (
              <span
                className="inline-flex items-center gap-1 rounded border border-border bg-muted px-1.5 py-0.5 text-xs"
                key={item}
              >
                {labelForOption(options, item)}
                <span
                  aria-label={`Remove ${labelForOption(options, item)}`}
                  className="rounded hover:bg-accent"
                  onClick={(event) => {
                    event.stopPropagation();
                    onChange(value.filter((selected) => selected !== item));
                  }}
                  onKeyDown={(event) => {
                    if (event.key !== "Enter" && event.key !== " ") return;
                    event.preventDefault();
                    event.stopPropagation();
                    onChange(value.filter((selected) => selected !== item));
                  }}
                  role="button"
                  tabIndex={0}
                >
                  <XIcon className="size-3" />
                </span>
              </span>
            ))
          )}
        </span>
        <ChevronDownIcon className="size-4 shrink-0 text-muted-foreground" />
      </Button>

      {open ? (
        <div className="absolute left-0 right-0 z-50 mt-1 overflow-hidden rounded-md border border-border bg-card shadow-lg">
          <div className="max-h-72 overflow-auto p-1">
            {options.map((option) => {
              const selected = value.includes(option.value);
              return (
                <button
                  className={cn(
                    "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm outline-none hover:bg-accent",
                    selected && "text-foreground",
                  )}
                  key={option.value}
                  onClick={() => toggleOption(option.value)}
                  type="button"
                >
                  <span
                    className={cn(
                      "flex size-4 items-center justify-center rounded border border-border",
                      selected && "border-primary bg-primary text-primary-foreground",
                    )}
                  >
                    {selected ? <CheckIcon className="size-3" /> : null}
                  </span>
                  <span>{option.label}</span>
                </button>
              );
            })}
          </div>
        </div>
      ) : null}
    </div>
  );
}

function labelForOption(options: MultiSelectOption[], value: string) {
  return options.find((option) => option.value === value)?.label ?? value;
}

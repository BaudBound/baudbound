import type * as React from "react";

import { cn } from "@/lib/utils";

type InputProps = Omit<React.ComponentProps<"input">, "autoComplete">;

export function Input({ className, ...props }: InputProps) {
  return (
    <input
      autoComplete="off"
      className={cn(
        "min-h-9 rounded-md border border-border bg-[#080b12] px-3 py-2 text-sm text-foreground outline-none transition-colors placeholder:text-muted-foreground focus:border-ring focus:ring-2 focus:ring-ring/25 disabled:opacity-60",
        className,
      )}
      {...props}
    />
  );
}

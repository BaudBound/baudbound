import type * as React from "react";

import { cn } from "@/lib/utils";

type TextareaProps = Omit<React.ComponentProps<"textarea">, "autoComplete">;

export function Textarea({ className, ...props }: TextareaProps) {
  return (
    <textarea
      autoComplete="off"
      className={cn(
        "rounded-md border border-border bg-[#080b12] px-3 py-2 text-sm text-foreground outline-none transition-colors placeholder:text-muted-foreground focus:border-ring focus:ring-2 focus:ring-ring/25 disabled:opacity-60",
        className,
      )}
      {...props}
    />
  );
}

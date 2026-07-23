import { Switch as SwitchPrimitive } from "radix-ui";
import type * as React from "react";

import { cn } from "@/lib/utils";

export function Switch({
  className,
  size = "default",
  ...props
}: React.ComponentProps<typeof SwitchPrimitive.Root> & {
  size?: "sm" | "default";
}) {
  return (
    <SwitchPrimitive.Root
      className={cn(
        "group/switch relative inline-flex shrink-0 cursor-pointer items-center rounded-full border border-border bg-border outline-none transition-[background-color,border-color,box-shadow] focus-visible:border-ring focus-visible:shadow-[0_0_0_2px_rgb(230_45_62_/_0.16)] disabled:cursor-not-allowed disabled:opacity-50 data-[state=checked]:border-primary data-[state=checked]:bg-primary data-[state=unchecked]:hover:border-accent data-[state=unchecked]:hover:bg-accent",
        size === "sm" ? "h-5 w-9" : "h-6 w-11",
        className,
      )}
      data-size={size}
      data-slot="switch"
      {...props}
    >
      <SwitchPrimitive.Thumb
        className={cn(
          "pointer-events-none block rounded-full bg-foreground shadow-sm transition-transform data-[state=checked]:bg-white data-[state=unchecked]:translate-x-0.5",
          size === "sm"
            ? "size-4 data-[state=checked]:translate-x-4"
            : "size-5 data-[state=checked]:translate-x-5",
        )}
        data-slot="switch-thumb"
      />
    </SwitchPrimitive.Root>
  );
}

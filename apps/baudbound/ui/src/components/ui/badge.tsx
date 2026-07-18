import { cva, type VariantProps } from "class-variance-authority";
import type * as React from "react";

import { cn } from "@/lib/utils";

const badgeVariants = cva(
  "inline-flex min-h-5 w-fit max-w-full shrink-0 items-center justify-center overflow-hidden text-ellipsis whitespace-nowrap rounded-full border px-2 text-center text-xs font-medium leading-4",
  {
    variants: {
      variant: {
        default: "border-border bg-muted text-foreground",
        destructive:
          "border-destructive/35 bg-destructive/10 text-destructive",
        good: "border-baud-green/35 bg-baud-green/10 text-baud-green",
        medium: "border-baud-amber/35 bg-baud-amber/10 text-baud-amber",
        muted: "border-border bg-card text-muted-foreground",
        red: "border-baud-red/35 bg-baud-red/10 text-baud-red",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
);

export function Badge({
  className,
  variant,
  ...props
}: React.ComponentProps<"span"> & VariantProps<typeof badgeVariants>) {
  return <span className={cn(badgeVariants({ variant }), className)} {...props} />;
}

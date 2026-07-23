import { cva, type VariantProps } from "class-variance-authority";
import type * as React from "react";

import { cn } from "@/lib/utils";

const buttonVariants = cva(
  "inline-flex h-9 shrink-0 items-center justify-center gap-1.5 rounded-md border border-transparent px-3 text-sm font-medium whitespace-nowrap outline-none transition-colors focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/45 disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:size-4 [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default: "bg-primary text-primary-foreground hover:bg-primary/85",
        destructive:
          "border-destructive/25 bg-destructive/10 text-destructive hover:bg-destructive/20",
        outline:
          "border-border bg-background hover:bg-muted hover:text-foreground",
        secondary:
          "border-border bg-secondary text-secondary-foreground hover:bg-accent",
        subtle:
          "border-border/70 bg-card text-muted-foreground hover:bg-muted hover:text-foreground",
        tab:
          "h-9 w-full justify-start rounded-md border-border/0 bg-transparent text-muted-foreground hover:bg-muted hover:text-foreground data-[active=true]:border-border data-[active=true]:bg-muted data-[active=true]:text-foreground",
      },
      size: {
        default: "h-9 px-3",
        sm: "h-7 px-2 text-xs",
        lg: "h-10 px-4",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

export function Button({
  className,
  variant,
  size,
  ...props
}: React.ComponentProps<"button"> & VariantProps<typeof buttonVariants>) {
  return (
    <button
      className={cn(buttonVariants({ variant, size }), className)}
      type={props.type ?? "button"}
      {...props}
    />
  );
}

import { cn } from "@/lib/utils";

export function CodeBlock({
  children,
  className,
}: {
  children: string;
  className?: string;
}) {
  return (
    <pre
      className={cn(
        "max-h-[360px] overflow-auto rounded-md border border-border bg-background p-3 font-mono text-xs leading-5 text-foreground",
        className,
      )}
    >
      <code>{children}</code>
    </pre>
  );
}

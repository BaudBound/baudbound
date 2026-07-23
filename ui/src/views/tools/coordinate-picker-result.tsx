import { Copy } from "lucide-react";
import type { ReactNode } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import type { CoordinatePickerResult } from "@/lib/runner-api";

export function CoordinatePickerResultView({ result }: { result: CoordinatePickerResult }) {
  return (
    <section className="grid gap-3 border-t border-border pt-3" aria-labelledby="selected-point">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h3 className="text-sm font-medium" id="selected-point">
          Selected point
        </h3>
        <CopyValueButton label="Copy coordinate pair" value={`${result.x}, ${result.y}`} />
      </div>
      <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
        <ResultFact label="X coordinate" value={String(result.x)} copyLabel="Copy X coordinate" />
        <ResultFact label="Y coordinate" value={String(result.y)} copyLabel="Copy Y coordinate" />
        <ResultFact label="Monitor" value={result.monitor.device_name} />
        <ResultFact label="Pixel color" value={result.color.hex} copyLabel="Copy pixel color">
          <span
            aria-hidden="true"
            className="size-4 shrink-0 rounded-sm border border-white/20"
            style={{ backgroundColor: result.color.hex }}
          />
        </ResultFact>
      </div>
    </section>
  );
}

function ResultFact({
  children,
  copyLabel,
  label,
  value,
}: {
  children?: ReactNode;
  copyLabel?: string;
  label: string;
  value: string;
}) {
  return (
    <div className="min-w-0 rounded-md border border-border bg-background px-3 py-2">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="mt-1 flex min-w-0 items-center gap-2">
        {children}
        <code className="min-w-0 flex-1 truncate text-sm text-foreground" title={value}>
          {value}
        </code>
        {copyLabel ? <CopyValueButton label={copyLabel} value={value} /> : null}
      </div>
    </div>
  );
}

function CopyValueButton({ label, value }: { label: string; value: string }) {
  async function copy() {
    try {
      await navigator.clipboard.writeText(value);
      toast.success(`${label.replace(/^Copy /, "")} copied.`);
    } catch (error) {
      toast.error(`Could not copy the value: ${String(error)}`);
    }
  }

  return (
    <Button
      aria-label={label}
      className="size-8 px-0"
      onClick={() => void copy()}
      title={label}
      variant="outline"
    >
      <Copy />
    </Button>
  );
}

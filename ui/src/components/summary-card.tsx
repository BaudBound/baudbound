import { Activity, CheckCircle2, ClipboardCheck } from "lucide-react";

import { Card, CardContent } from "@/components/ui/card";

export function SummaryCard({ label, value }: { label: string; value: number }) {
  const Icon =
    label === "Problems"
      ? Activity
      : label === "Enabled"
        ? CheckCircle2
        : ClipboardCheck;

  return (
    <Card>
      <CardContent className="flex items-center justify-between gap-3">
        <div>
          <div className="text-sm text-muted-foreground">{label}</div>
          <div className="mt-1 text-2xl font-semibold">{value}</div>
        </div>
        <Icon className="size-5 text-muted-foreground" />
      </CardContent>
    </Card>
  );
}

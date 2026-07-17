import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";

type StatusTone = "destructive" | "good" | "medium" | "muted";

export function StatusSummaryCard({
  label,
  tone = "muted",
  value,
}: {
  label: string;
  tone?: StatusTone;
  value: number;
}) {
  return (
    <Card className="min-w-0">
      <CardContent className="flex min-w-0 flex-wrap items-start justify-between gap-x-3 gap-y-2">
        <div className="min-w-0">
          <div className="break-words text-sm text-muted-foreground">{label}</div>
          <div className="mt-1 text-2xl font-semibold">{value}</div>
        </div>
        <Badge className="max-w-full shrink-0" variant={tone}>
          {label}
        </Badge>
      </CardContent>
    </Card>
  );
}

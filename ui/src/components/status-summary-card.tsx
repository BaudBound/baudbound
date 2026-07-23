import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";

type StatusTone = "destructive" | "good" | "medium" | "muted";

export function StatusSummaryCard({
  badgeLabel,
  label,
  tone = "muted",
  value,
}: {
  badgeLabel?: string;
  label: string;
  tone?: StatusTone;
  value: number;
}) {
  return (
    <Card className="min-w-0">
      <CardContent className="grid min-w-0 grid-cols-[minmax(0,1fr)_auto] items-start gap-3">
        <div className="min-w-0">
          <div
            className="overflow-hidden text-ellipsis whitespace-nowrap text-sm text-muted-foreground"
            title={label}
          >
            {label}
          </div>
          <div className="mt-1 text-2xl font-semibold">{value}</div>
        </div>
        <Badge variant={tone}>{badgeLabel ?? label}</Badge>
      </CardContent>
    </Card>
  );
}

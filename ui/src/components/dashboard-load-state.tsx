import { RefreshCw } from "lucide-react";

import { EmptyState } from "@/components/empty-state";
import { Button } from "@/components/ui/button";

export function DashboardLoadState({
  error,
  onRetry,
}: {
  error: string | null;
  onRetry: () => void;
}) {
  if (!error) {
    return <EmptyState>Loading runner state...</EmptyState>;
  }

  return (
    <EmptyState>
      <div className="mx-auto flex max-w-2xl flex-col items-center gap-3">
        <div>
          <div className="font-medium text-foreground">Unable to load runner state</div>
          <div className="mt-1 break-words text-sm">{error}</div>
        </div>
        <Button onClick={onRetry} variant="outline">
          <RefreshCw />
          Retry
        </Button>
      </div>
    </EmptyState>
  );
}

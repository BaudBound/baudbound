import { FileUp, RefreshCw } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import type { DashboardAction } from "@/lib/app-types";
import { RemotePackageDialog } from "@/views/remote-package-dialog";

export function ScriptPackageToolbar({
  busyActions,
  canCheckUpdates,
  onCheckUpdates,
  runAction,
}: {
  busyActions: Set<string>;
  canCheckUpdates: boolean;
  onCheckUpdates: () => void;
  runAction: DashboardAction;
}) {
  const [dialogOperation, setDialogOperation] = useState<"import" | "update" | null>(null);
  return (
    <>
    <Card>
      <CardContent className="grid gap-4 md:grid-cols-[minmax(0,1fr)_auto] md:items-start">
        <div className="min-w-0">
          <div className="text-sm font-medium">Package management</div>
          <div className="text-xs text-muted-foreground">
            Import packages or update installed scripts. Trigger registrations refresh
            automatically.
          </div>
        </div>
        <div className="grid gap-2 sm:grid-cols-3 md:flex md:justify-end">
          <Button
            className="w-full md:w-auto"
            disabled={!canCheckUpdates}
            onClick={onCheckUpdates}
            variant="outline"
          >
            <RefreshCw />
            Check updates
          </Button>
          <Button
            className="w-full md:w-auto"
            disabled={busyActions.has("import-package")}
            onClick={() => setDialogOperation("import")}
            variant="secondary"
          >
            <FileUp />
            {busyActions.has("import-package") ? "Working..." : "Import"}
          </Button>
          <Button
            className="w-full md:w-auto"
            disabled={busyActions.has("update-package")}
            onClick={() => setDialogOperation("update")}
            variant="outline"
          >
            <FileUp />
            {busyActions.has("update-package") ? "Working..." : "Update"}
          </Button>
        </div>
      </CardContent>
    </Card>
    {dialogOperation ? (
      <RemotePackageDialog
        busyActions={busyActions}
        onOpenChange={(open) => {
          if (!open) setDialogOperation(null);
        }}
        open
        operation={dialogOperation}
        runAction={runAction}
      />
    ) : null}
    </>
  );
}

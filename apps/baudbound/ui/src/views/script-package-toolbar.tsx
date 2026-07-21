import { FileUp } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import type { DashboardAction } from "@/lib/app-types";
import {
  type ActionPayload,
  type PackageFileSelection,
  importScriptPackage,
  selectPackageFile,
  updateScriptPackage,
} from "@/lib/runner-api";

export function ScriptPackageToolbar({
  busyActions,
  runAction,
}: {
  busyActions: Set<string>;
  runAction: DashboardAction;
}) {
  return (
    <Card>
      <CardContent className="grid gap-4 md:grid-cols-[minmax(0,1fr)_auto] md:items-start">
        <div className="min-w-0">
          <div className="text-sm font-medium">Package management</div>
          <div className="text-xs text-muted-foreground">
            Import packages or update installed scripts. Trigger registrations refresh
            automatically.
          </div>
        </div>
        <div className="grid grid-cols-2 gap-2 md:flex md:justify-end">
          <Button
            className="w-full md:w-auto"
            disabled={busyActions.has("import-package")}
            onClick={() =>
              selectAndRunPackageAction("import-package", runAction, importScriptPackage)
            }
            variant="secondary"
          >
            <FileUp />
            {busyActions.has("import-package") ? "Working..." : "Import"}
          </Button>
          <Button
            className="w-full md:w-auto"
            disabled={busyActions.has("update-package")}
            onClick={() =>
              selectAndRunPackageAction("update-package", runAction, updateScriptPackage)
            }
            variant="outline"
          >
            <FileUp />
            {busyActions.has("update-package") ? "Working..." : "Update"}
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}

async function selectAndRunPackageAction(
  actionId: string,
  runAction: DashboardAction,
  action: (selection: PackageFileSelection) => Promise<ActionPayload>,
) {
  const operation = actionId === "import-package" ? "import" : "update";
  let selection: PackageFileSelection | null;
  try {
    selection = await selectPackageFile(operation);
  } catch (error) {
    toast.error(`Package selection failed: ${String(error)}`);
    return;
  }
  if (!selection) return;
  await runAction(actionId, () => action(selection));
}

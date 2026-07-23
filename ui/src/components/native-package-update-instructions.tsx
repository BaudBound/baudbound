import { Copy } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import {
  installationTypeLabel,
  LINUX_INSTALL_COMMAND,
  type AppInstallationType,
} from "@/lib/update-policy";

export function NativePackageUpdateInstructions({
  installationType,
}: {
  installationType: AppInstallationType;
}) {
  return (
    <section className="grid gap-3 rounded-md border border-border bg-background p-3">
      <div className="grid gap-1 text-sm">
        <h3 className="font-medium">Update the {installationTypeLabel(installationType)}</h3>
        <p className="text-muted-foreground">
          Stop active runs and the background runner, then fully quit BaudBound, including the
          tray application. Run this command in a terminal and approve any prompt from APT or DNF.
          Open BaudBound again after the command finishes.
        </p>
      </div>
      <div className="flex min-w-0 items-stretch gap-2">
        <code className="min-w-0 flex-1 select-text overflow-x-auto rounded-md border border-border bg-muted px-3 py-2 text-xs">
          {LINUX_INSTALL_COMMAND}
        </code>
        <Button
          aria-label="Copy update command"
          onClick={() => {
            void navigator.clipboard.writeText(LINUX_INSTALL_COMMAND).then(
              () => toast.success("Update command copied."),
              (error) => toast.error(`Could not copy update command: ${String(error)}`),
            );
          }}
          size="sm"
          variant="outline"
        >
          <Copy />
          Copy
        </Button>
      </div>
    </section>
  );
}

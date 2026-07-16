import { CheckCircle2, Download, RefreshCw, TriangleAlert } from "lucide-react";
import { useCallback } from "react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useAppUpdater } from "@/hooks/use-app-updater";
import { updateProgressPercent } from "@/lib/update-progress";

export function AppUpdateDialog({
  automaticCheck,
  onError,
}: {
  automaticCheck: boolean;
  onError: (message: string) => void;
}) {
  const stableErrorHandler = useCallback((message: string) => onError(message), [onError]);
  const { dismiss, download, installAndRestart, retry, state } = useAppUpdater(
    stableErrorHandler,
    automaticCheck,
  );
  const open =
    state.phase === "available" ||
    state.phase === "downloading" ||
    state.phase === "ready" ||
    state.phase === "error";
  const canDismiss = state.phase === "available" || state.phase === "error";
  const progressPercent = updateProgressPercent(state.progress);

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => !nextOpen && canDismiss && dismiss()}>
      <DialogContent
        className="w-[min(calc(100vw-2rem),540px)]"
        showCloseButton={canDismiss}
      >
        <DialogHeader>
          <DialogTitle>{dialogTitle(state.phase, state.version)}</DialogTitle>
          <DialogDescription>{dialogDescription(state.phase)}</DialogDescription>
        </DialogHeader>

        {state.phase === "available" && state.currentVersion ? (
          <div className="text-sm text-muted-foreground">
            Installed {state.currentVersion} <span aria-hidden="true">-&gt;</span> Available {state.version}
          </div>
        ) : null}

        {state.phase === "available" && state.releaseNotes ? (
          <div className="max-h-48 overflow-y-auto whitespace-pre-wrap rounded-md border border-border bg-background p-3 text-sm leading-6 text-muted-foreground">
            {state.releaseNotes}
          </div>
        ) : null}

        {state.phase === "downloading" ? (
          <div className="grid gap-2" aria-live="polite">
            <div className="flex items-center justify-between gap-3 text-sm">
              <span className="text-muted-foreground">Downloading and verifying update</span>
              <span className="font-medium">
                {progressLabel(progressPercent, state.progress.downloadedBytes)}
              </span>
            </div>
            <div
              aria-label="Update download progress"
              aria-valuemax={100}
              aria-valuemin={0}
              aria-valuenow={progressPercent ?? undefined}
              className="h-2 overflow-hidden rounded-sm bg-muted"
              role="progressbar"
            >
              <div
                className={
                  progressPercent === null
                    ? "h-full w-1/3 animate-pulse bg-primary"
                    : "h-full bg-primary transition-[width]"
                }
                style={progressPercent === null ? undefined : { width: `${progressPercent}%` }}
              />
            </div>
          </div>
        ) : null}

        {state.phase === "ready" ? (
          <div className="flex items-start gap-3 rounded-md border border-baud-green/25 bg-baud-green/10 p-3 text-sm text-baud-green">
            <CheckCircle2 className="mt-0.5 size-4 shrink-0" />
            <span>The signed update is downloaded and verified. Restart to install it.</span>
          </div>
        ) : null}

        {state.phase === "error" ? (
          <div className="flex items-start gap-3 rounded-md border border-destructive/25 bg-destructive/10 p-3 text-sm text-destructive">
            <TriangleAlert className="mt-0.5 size-4 shrink-0" />
            <span className="min-w-0 break-words">{state.error}</span>
          </div>
        ) : null}

        <DialogFooter>
          {canDismiss ? (
            <Button onClick={dismiss} variant="outline">Later</Button>
          ) : null}
          {state.phase === "available" ? (
            <Button onClick={() => void download()}>
              <Download />
              Download update
            </Button>
          ) : null}
          {state.phase === "error" ? (
            <Button onClick={() => void retry()}>
              <RefreshCw />
              Try again
            </Button>
          ) : null}
          {state.phase === "ready" ? (
            <Button onClick={() => void installAndRestart()}>
              <RefreshCw />
              Restart and install
            </Button>
          ) : null}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function dialogTitle(phase: string, version: string | null) {
  if (phase === "available") return `BaudBound ${version ?? "update"} is available`;
  if (phase === "downloading") return `Installing BaudBound ${version ?? "update"}`;
  if (phase === "ready") return "Update ready";
  return "BaudBound could not update";
}

function dialogDescription(phase: string) {
  if (phase === "available") {
    return "Review the release notes, then download the signed update.";
  }
  if (phase === "downloading") {
    return "Keep BaudBound open while the update is downloaded and verified.";
  }
  if (phase === "ready") return "Your running scripts will stop when the application restarts.";
  return "The current version is still installed and can continue running normally.";
}

function progressLabel(percent: number | null, downloadedBytes: number) {
  if (percent !== null) return `${percent}%`;
  if (downloadedBytes === 0) return "Starting...";
  return `${(downloadedBytes / 1_048_576).toFixed(1)} MB`;
}

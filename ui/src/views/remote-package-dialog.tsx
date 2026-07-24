import { listen } from "@tauri-apps/api/event";
import { FileUp, Globe2, ShieldCheck } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";

import { Details } from "@/components/details";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import type { DashboardAction } from "@/lib/app-types";
import {
  cancelRemoteScriptPackagePreparation,
  discardRemotePackageReview,
  importScriptPackage,
  installRemoteScriptPackage,
  prepareDiscoveredScriptUpdate,
  prepareRemoteScriptPackage,
  remotePackageProgressEvent,
  type RemotePackageOperation,
  type RemotePackageReview,
  type RemotePreparationProgress,
  selectPackageFile,
  updateScriptPackage,
} from "@/lib/runner-api";

export function RemotePackageDialog({
  busyActions,
  onOpenChange,
  open,
  operation,
  runAction,
  discoveredScriptId,
  onInstalled,
  preparedReview,
}: {
  busyActions: Set<string>;
  onOpenChange: (open: boolean) => void;
  open: boolean;
  operation: RemotePackageOperation;
  runAction: DashboardAction;
  discoveredScriptId?: string;
  onInstalled?: (scriptId: string) => void;
  preparedReview?: RemotePackageReview;
}) {
  const [mode, setMode] = useState<"choice" | "remote">("choice");
  const [url, setUrl] = useState("");
  const [preparing, setPreparing] = useState(false);
  const [review, setReview] = useState<RemotePackageReview | null>(preparedReview ?? null);
  const [progress, setProgress] = useState<RemotePreparationProgress | null>(null);
  const [cancelling, setCancelling] = useState(false);
  const [prepareError, setPrepareError] = useState<string | null>(null);
  const discoveryRequest = useRef<string | null>(null);
  const preparationRequest = useRef<string | null>(null);
  const closeAfterCancellation = useRef(false);
  const installedReview = useRef<string | null>(null);
  const reviewRef = useRef<RemotePackageReview | null>(preparedReview ?? null);
  const actionId = `${operation}-package`;
  const installing = busyActions.has(`${operation}-remote-package`);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<RemotePreparationProgress>(remotePackageProgressEvent, ({ payload }) => {
      if (payload.request_id === preparationRequest.current) setProgress(payload);
    }).then((dispose) => {
      unlisten = dispose;
    });
    return () => unlisten?.();
  }, []);

  useEffect(() => {
    if (!open || !preparedReview) return;
    setMode("remote");
    storeReview(preparedReview);
    setProgress(null);
  }, [open, preparedReview]);

  useEffect(() => {
    if (!open || !discoveredScriptId || preparedReview) return;
    if (discoveryRequest.current === discoveredScriptId) return;
    discoveryRequest.current = discoveredScriptId;
    setMode("remote");
    setPrepareError(null);
    setPreparing(true);
    const requestId = crypto.randomUUID();
    preparationRequest.current = requestId;
    void prepareDiscoveredScriptUpdate(discoveredScriptId, requestId)
      .then(storeReview)
      .catch((error) => {
        const message = String(error);
        if (!isCancellation(message)) {
          setPrepareError(message);
          toast.error(`Remote update preparation failed: ${message}`);
        }
      })
      .finally(() => finishPreparation());
  }, [discoveredScriptId, open, preparedReview]);

  function finishPreparation() {
    preparationRequest.current = null;
    setPreparing(false);
    setCancelling(false);
    if (closeAfterCancellation.current) {
      closeAfterCancellation.current = false;
      resetDialog();
      onOpenChange(false);
    }
  }

  function resetDialog() {
    const pendingReview = reviewRef.current;
    if (pendingReview && installedReview.current !== pendingReview.review_id) {
      void discardRemotePackageReview(pendingReview.review_id);
    }
    setMode("choice");
    setUrl("");
    setReview(null);
    reviewRef.current = null;
    setPreparing(false);
    setCancelling(false);
    setProgress(null);
    setPrepareError(null);
    discoveryRequest.current = null;
    preparationRequest.current = null;
    installedReview.current = null;
  }

  function handleOpenChange(nextOpen: boolean) {
    if (installing) return;
    if (!nextOpen) {
      if (preparing) {
        closeAfterCancellation.current = true;
        void cancelPreparation();
        return;
      }
      resetDialog();
    }
    onOpenChange(nextOpen);
  }

  async function cancelPreparation() {
    const requestId = preparationRequest.current;
    if (!requestId || cancelling) return;
    setCancelling(true);
    try {
      await cancelRemoteScriptPackagePreparation(requestId);
    } catch (error) {
      setCancelling(false);
      toast.error(`Could not cancel the download: ${String(error)}`);
    }
  }

  async function selectLocalFile() {
    try {
      const selection = await selectPackageFile(operation);
      if (!selection) return;
      const succeeded = await runAction(actionId, () =>
        operation === "import"
          ? importScriptPackage(selection)
          : updateScriptPackage(selection),
      );
      if (succeeded) handleOpenChange(false);
    } catch (error) {
      toast.error(`Package selection failed: ${String(error)}`);
    }
  }

  async function prepareRemote() {
    if (!url.trim() || preparing) return;
    setPreparing(true);
    setProgress(null);
    setPrepareError(null);
    const requestId = crypto.randomUUID();
    preparationRequest.current = requestId;
    try {
      storeReview(
        await prepareRemoteScriptPackage(operation, requestId, "package", url.trim()),
      );
    } catch (error) {
      const message = String(error);
      if (!isCancellation(message)) {
        setPrepareError(message);
        toast.error(`Remote package preparation failed: ${message}`);
      }
    } finally {
      finishPreparation();
    }
  }

  async function installReviewed() {
    if (!review) return;
    const succeeded = await runAction(`${operation}-remote-package`, () =>
      installRemoteScriptPackage(review),
    );
    if (succeeded) {
      installedReview.current = review.review_id;
      onInstalled?.(review.script_id);
      handleOpenChange(false);
    }
  }

  function storeReview(nextReview: RemotePackageReview) {
    reviewRef.current = nextReview;
    setReview(nextReview);
  }

  return (
    <Dialog onOpenChange={handleOpenChange} open={open}>
      <DialogContent className="w-[min(calc(100vw-2rem),720px)]">
        <DialogHeader>
          <DialogTitle>{operation === "import" ? "Import script" : "Update script"}</DialogTitle>
          <DialogDescription>
            Choose a local package or download one from a public HTTPS address. Remote packages are validated before installation.
          </DialogDescription>
        </DialogHeader>

        {preparing ? (
          <PreparationProgress progress={progress} />
        ) : prepareError ? (
          <div className="rounded-md border border-destructive/40 bg-destructive/10 p-4 text-sm text-destructive">
            {prepareError}
          </div>
        ) : mode === "choice" ? (
          <div className="grid gap-3 sm:grid-cols-2">
            <Button
              className="h-auto items-start justify-start gap-3 p-4 text-left"
              disabled={busyActions.has(actionId)}
              onClick={selectLocalFile}
              variant="outline"
            >
              <FileUp className="mt-0.5" />
              <span>
                <span className="block font-medium">Choose file</span>
                <span className="mt-1 block text-xs font-normal text-muted-foreground">
                  Select a .bbs package stored on this computer.
                </span>
              </span>
            </Button>
            <Button
              className="h-auto items-start justify-start gap-3 p-4 text-left"
              onClick={() => setMode("remote")}
              variant="outline"
            >
              <Globe2 className="mt-0.5" />
              <span>
                <span className="block font-medium">Use URL</span>
                <span className="mt-1 block text-xs font-normal text-muted-foreground">
                  Download from a public HTTPS address.
                </span>
              </span>
            </Button>
          </div>
        ) : review ? (
          <div className="grid gap-4">
            <div className="rounded-md border border-border p-4">
              <div className="mb-3 flex flex-wrap items-center gap-2">
                <ShieldCheck className="size-4 text-baud-green" />
                <span className="font-medium">Package ready for review</span>
                <Badge variant="medium">{review.risk_level} risk</Badge>
              </div>
              <Details
                rows={[
                  ["Name", review.script_name],
                  ["Script ID", review.script_id],
                  ["Current version", review.current_version ?? "Not installed"],
                  ["New version", review.version],
                  ["Target runtimes", review.target_runtime],
                  ["Size", formatBytes(review.size)],
                  ["SHA-256", review.sha256],
                ]}
              />
            </div>
            <div className="grid gap-3 sm:grid-cols-2">
              <ReviewList label="Permissions" values={review.permissions} />
              <ReviewList label="Capabilities" values={review.capabilities} />
            </div>
            <p className="text-xs text-muted-foreground">
              Installation does not approve or run this package. Review its declared access again after installation.
            </p>
          </div>
        ) : (
          <div className="grid gap-4">
            <label className="grid gap-1.5 text-sm" htmlFor="remote-package-url">
              HTTPS URL
              <Input
                autoComplete="off"
                id="remote-package-url"
                onChange={(event) => setUrl(event.target.value)}
                placeholder="https://example.com/releases/script.bbs"
                spellCheck={false}
                value={url}
              />
            </label>
            <p className="text-xs text-muted-foreground">
              BaudBound blocks local and private network destinations, unsafe redirects, oversized responses, and non-HTTPS downloads.
            </p>
          </div>
        )}

        <DialogFooter>
          <Button
            disabled={installing || cancelling}
            onClick={() => preparing ? void cancelPreparation() : handleOpenChange(false)}
            variant="outline"
          >
            {cancelling ? "Cancelling..." : preparing ? "Cancel download" : "Cancel"}
          </Button>
          {mode === "remote" && !review && !discoveredScriptId ? (
            <Button disabled={!url.trim() || preparing} onClick={prepareRemote}>
              {preparing ? "Downloading..." : "Download and review"}
            </Button>
          ) : null}
          {review ? (
            <Button disabled={installing} onClick={installReviewed}>
              {installing ? "Installing..." : operation === "import" ? "Install script" : "Install update"}
            </Button>
          ) : null}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function ReviewList({ label, values }: { label: string; values: string[] }) {
  return (
    <section className="rounded-md border border-border p-3">
      <h3 className="mb-2 text-sm font-medium">{label}</h3>
      {values.length ? (
        <div className="flex flex-wrap gap-1.5">
          {values.map((value) => (
            <Badge key={value} variant="muted">{value}</Badge>
          ))}
        </div>
      ) : (
        <p className="text-xs text-muted-foreground">None declared.</p>
      )}
    </section>
  );
}

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KiB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MiB`;
}

function PreparationProgress({ progress }: { progress: RemotePreparationProgress | null }) {
  const total = progress?.total_bytes ?? null;
  const percent = progress && total && total > 0
    ? Math.min(100, Math.round((progress.transferred_bytes / total) * 100))
    : null;
  return (
    <section className="grid gap-3 rounded-md border border-border p-4">
      <div className="flex flex-wrap items-center justify-between gap-2 text-sm">
        <span>{progress ? preparationStageLabel(progress.stage) : "Starting secure download"}</span>
        {progress ? (
          <span className="text-xs text-muted-foreground">
            {formatBytes(progress.transferred_bytes)}{total ? ` of ${formatBytes(total)}` : ""}
          </span>
        ) : null}
      </div>
      <div
        aria-label="Remote package preparation progress"
        aria-valuemax={100}
        aria-valuemin={0}
        aria-valuenow={percent ?? undefined}
        className="h-2 overflow-hidden rounded-sm bg-muted"
        role="progressbar"
      >
        <div
          className={`h-full bg-primary transition-[width] ${percent === null ? "w-1/3 animate-pulse" : ""}`}
          style={percent === null ? undefined : { width: `${percent}%` }}
        />
      </div>
    </section>
  );
}

function preparationStageLabel(stage: RemotePreparationProgress["stage"]) {
  if (stage === "downloading_package") return "Downloading script package";
  if (stage === "downloading_repository") return "Downloading script repository";
  if (stage === "verifying_hash") return "Verifying package hash";
  if (stage === "validating_package") return "Validating package contents";
  return "Preparing package review";
}

function isCancellation(message: string) {
  return message.toLowerCase().includes("cancelled");
}

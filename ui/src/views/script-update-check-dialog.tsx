import { listen } from "@tauri-apps/api/event";
import { AlertCircle, CheckCircle2, RefreshCw } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

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
import type { DashboardAction } from "@/lib/app-types";
import { runWithConcurrency } from "@/lib/bounded-concurrency";
import {
  checkScriptUpdate,
  checkScriptUpdates,
  cancelRemoteScriptPackagePreparation,
  prepareDiscoveredScriptUpdate,
  remotePackageProgressEvent,
  type DashboardPayload,
  type ScriptStatus,
  type ScriptUpdateState,
  type RemotePackageReview,
  type RemotePreparationProgress,
} from "@/lib/runner-api";
import { RemotePackageDialog } from "@/views/remote-package-dialog";

type CheckStatus =
  | "available"
  | "checking"
  | "current"
  | "failed"
  | "pending"
  | "preparing"
  | "ready"
  | "unconfigured"
  | "updated";

type CheckRow = {
  error: string | null;
  latestVersion: string | null;
  preparation: "failed" | "idle" | "preparing" | "ready";
  preparationProgress: RemotePreparationProgress | null;
  preparationRequestId: string | null;
  review: RemotePackageReview | null;
  script: ScriptStatus;
  status: CheckStatus;
};

const checkConcurrency = 3;
const downloadConcurrency = 3;

export function ScriptUpdateCheckDialog({
  busyActions,
  dashboard,
  onDashboard,
  onOpenChange,
  open,
  runAction,
}: {
  busyActions: Set<string>;
  dashboard: DashboardPayload;
  onDashboard: (dashboard: DashboardPayload) => void;
  onOpenChange: (open: boolean) => void;
  open: boolean;
  runAction: DashboardAction;
}) {
  const [rows, setRows] = useState<CheckRow[]>([]);
  const [reviewQueue, setReviewQueue] = useState<string[]>([]);
  const initialized = useRef(false);
  const completionReported = useRef(false);
  const preparationBatchCancelled = useRef(false);

  useEffect(() => {
    if (!open) {
      initialized.current = false;
      completionReported.current = false;
      preparationBatchCancelled.current = true;
      setReviewQueue([]);
      return;
    }
    if (initialized.current) return;
    initialized.current = true;
    const scripts = [...dashboard.runner.scripts].sort((left, right) =>
      left.installed.name.localeCompare(right.installed.name),
    );
    const initialRows = scripts.map<CheckRow>((script) => ({
      error: null,
      latestVersion: null,
      preparation: "idle",
      preparationProgress: null,
      preparationRequestId: null,
      review: null,
      script,
      status: script.metadata?.repository_url.trim() ? "pending" : "unconfigured",
    }));
    setRows(initialRows);
    void checkEligibleScripts(initialRows, onDashboard, setRows);
  }, [dashboard, onDashboard, open]);

  const eligibleCount = rows.filter((row) => row.status !== "unconfigured").length;
  const completedCount = rows.filter((row) =>
    ["available", "current", "failed", "updated"].includes(row.status),
  ).length;
  const availableRows = rows.filter((row) => row.status === "available");
  const failedCount = rows.filter((row) => row.status === "failed").length;
  const currentCount = rows.filter((row) =>
    ["current", "updated"].includes(row.status),
  ).length;
  const progress = eligibleCount === 0 ? 100 : Math.round((completedCount / eligibleCount) * 100);
  const activeUpdate = reviewQueue[0] ?? null;
  const activeScript = useMemo(
    () => rows.find((row) => row.script.installed.id === activeUpdate)?.script ?? null,
    [activeUpdate, rows],
  );
  const activeReview = useMemo(
    () => rows.find((row) => row.script.installed.id === activeUpdate)?.review ?? null,
    [activeUpdate, rows],
  );
  const preparingRows = rows.filter((row) => row.preparation === "preparing");

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<RemotePreparationProgress>(remotePackageProgressEvent, ({ payload }) => {
      setRows((current) => current.map((row) =>
        row.preparationRequestId === payload.request_id
          ? { ...row, preparationProgress: payload }
          : row,
      ));
    }).then((dispose) => {
      unlisten = dispose;
    });
    return () => unlisten?.();
  }, []);

  useEffect(() => {
    if (!open || completionReported.current || eligibleCount === 0) return;
    if (completedCount !== eligibleCount) return;
    completionReported.current = true;
    toast.success(
      `Update checks finished. ${availableRows.length} available, ${currentCount} current, ${failedCount} failed.`,
    );
  }, [availableRows.length, completedCount, currentCount, eligibleCount, failedCount, open]);

  function retry(scriptId: string) {
    const row = rows.find((candidate) => candidate.script.installed.id === scriptId);
    if (!row) return;
    setRows((current) => updateRow(current, scriptId, { error: null, status: "checking" }));
    void checkOne(row.script, onDashboard)
      .then((state) => {
        setRows((current) => applyCheckResult(current, scriptId, state));
      })
      .catch((error) => {
        setRows((current) =>
          updateRow(current, scriptId, { error: String(error), status: "failed" }),
        );
      });
  }

  function closeUpdateReview() {
    if (!activeUpdate) return;
    setReviewQueue((current) =>
      current[0] === activeUpdate ? current.slice(1) : current,
    );
    setRows((current) => updateRow(current, activeUpdate, {
      preparation: "idle",
      preparationProgress: null,
      review: null,
    }));
  }

  function prepareUpdates(scriptIds: string[]) {
    const uniqueIds = [...new Set(scriptIds)].filter((scriptId) => {
      const row = rows.find((candidate) => candidate.script.installed.id === scriptId);
      return row?.status === "available" && row.preparation !== "preparing" && !row.review;
    });
    if (!uniqueIds.length) return;
    preparationBatchCancelled.current = false;
    void prepareAvailableUpdates(
      uniqueIds,
      setRows,
      setReviewQueue,
      preparationBatchCancelled,
    );
  }

  async function cancelDownloads() {
    preparationBatchCancelled.current = true;
    await Promise.allSettled(
      preparingRows
        .map((row) => row.preparationRequestId)
        .filter((requestId): requestId is string => Boolean(requestId))
        .map(cancelRemoteScriptPackagePreparation),
    );
  }

  function handleOpenChange(nextOpen: boolean) {
    if (!nextOpen && (preparingRows.length > 0 || reviewQueue.length > 0)) return;
    onOpenChange(nextOpen);
  }

  return (
    <>
      <Dialog onOpenChange={handleOpenChange} open={open}>
        <DialogContent className="flex max-h-[min(88vh,860px)] w-[min(calc(100vw-2rem),900px)] flex-col overflow-hidden">
          <DialogHeader>
            <DialogTitle>Check script updates</DialogTitle>
            <DialogDescription>
              BaudBound checks every configured script repository. No package is installed or approved automatically.
            </DialogDescription>
          </DialogHeader>

          <section className="grid gap-2 rounded-md border border-border p-3">
            <div className="flex flex-wrap items-center justify-between gap-2 text-sm">
              <span>{completedCount} of {eligibleCount} configured scripts checked</span>
              <div className="flex flex-wrap gap-1.5">
                <Badge variant="medium">{availableRows.length} available</Badge>
                <Badge variant="good">{currentCount} current</Badge>
                <Badge variant={failedCount ? "destructive" : "muted"}>{failedCount} failed</Badge>
              </div>
            </div>
            <div
              aria-label="Script update check progress"
              aria-valuemax={100}
              aria-valuemin={0}
              aria-valuenow={progress}
              className="h-2 overflow-hidden rounded-sm bg-muted"
              role="progressbar"
            >
              <div className="h-full bg-primary transition-[width]" style={{ width: `${progress}%` }} />
            </div>
          </section>

          <div className="min-h-0 flex-1 overflow-y-auto rounded-md border border-border">
            {rows.map((row) => {
              const queued = reviewQueue.includes(row.script.installed.id);
              return (
                <div
                  className="grid gap-3 border-b border-border p-3 last:border-b-0 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center"
                  key={row.script.installed.id}
                >
                  <div className="min-w-0">
                    <div className="font-medium">{row.script.installed.name}</div>
                    <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                      <span>Installed {row.script.metadata?.version ?? "Unknown"}</span>
                      {row.latestVersion ? <span>Published {row.latestVersion}</span> : null}
                    </div>
                    {row.error ? (
                      <p className="mt-2 select-text break-words text-xs text-destructive">{row.error}</p>
                    ) : null}
                    {row.status === "checking" || row.preparation === "preparing" ? (
                      <div className="mt-2 h-1.5 overflow-hidden rounded-sm bg-muted">
                        <div
                          className={`h-full bg-primary ${preparationPercent(row.preparationProgress) === null ? "w-1/3 animate-pulse" : "transition-[width]"}`}
                          style={preparationPercent(row.preparationProgress) === null ? undefined : {
                            width: `${preparationPercent(row.preparationProgress)}%`,
                          }}
                        />
                      </div>
                    ) : null}
                    {row.preparation === "preparing" ? (
                      <p className="mt-1 text-xs text-muted-foreground">
                        {preparationSummary(row.preparationProgress)}
                      </p>
                    ) : null}
                  </div>
                  <div className="flex flex-wrap items-center gap-2 sm:justify-end">
                    <CheckStatusBadge
                      status={row.preparation === "preparing" ? "preparing" : queued ? "ready" : row.status}
                    />
                    {row.status === "failed" ? (
                      <Button onClick={() => retry(row.script.installed.id)} size="sm" variant="outline">
                        <RefreshCw />
                        Retry
                      </Button>
                    ) : null}
                    {row.status === "available" ? (
                      <Button
                        disabled={queued || row.preparation === "preparing"}
                        onClick={() => prepareUpdates([row.script.installed.id])}
                        size="sm"
                      >
                        Update
                      </Button>
                    ) : null}
                  </div>
                </div>
              );
            })}
          </div>

          <DialogFooter>
            <Button
              disabled={reviewQueue.length > 0 || preparingRows.length > 0}
              onClick={() => handleOpenChange(false)}
              variant="outline"
            >
              Close
            </Button>
            {preparingRows.length > 0 ? (
              <Button onClick={() => void cancelDownloads()} variant="outline">
                Cancel downloads
              </Button>
            ) : null}
            <Button
              disabled={availableRows.length === 0 || preparingRows.length > 0}
              onClick={() => prepareUpdates(availableRows.map((row) => row.script.installed.id))}
            >
              Update all
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      {activeScript && activeReview ? (
        <RemotePackageDialog
          busyActions={busyActions}
          onInstalled={(scriptId) => {
            setRows((current) => updateRow(current, scriptId, {
              preparation: "idle",
              preparationProgress: null,
              review: null,
              status: "updated",
            }));
            setReviewQueue((current) => current[0] === scriptId ? current.slice(1) : current);
          }}
          onOpenChange={(nextOpen) => {
            if (!nextOpen) closeUpdateReview();
          }}
          open
          operation="update"
          preparedReview={activeReview}
          runAction={runAction}
        />
      ) : null}
    </>
  );
}

async function checkEligibleScripts(
  initialRows: CheckRow[],
  onDashboard: (dashboard: DashboardPayload) => void,
  setRows: React.Dispatch<React.SetStateAction<CheckRow[]>>,
) {
  const eligible = initialRows.filter((row) => row.status === "pending");
  const grouped = new Map<string, CheckRow[]>();
  for (const row of eligible) {
    const repositoryUrl = row.script.metadata?.repository_url.trim();
    if (!repositoryUrl) continue;
    const group = grouped.get(repositoryUrl) ?? [];
    group.push(row);
    grouped.set(repositoryUrl, group);
  }
  await runWithConcurrency(
    [...grouped.values()],
    checkConcurrency,
    async (group) => {
      const scriptIds = group.map((row) => row.script.installed.id);
      setRows((current) =>
        scriptIds.reduce(
          (next, scriptId) =>
            updateRow(next, scriptId, { error: null, status: "checking" }),
          current,
        ),
      );
      try {
        const result = await checkScriptUpdates(scriptIds);
        onDashboard(result.dashboard);
        setRows((current) =>
          scriptIds.reduce((next, scriptId) => {
            const error = result.errors[scriptId];
            if (error) {
              return updateRow(next, scriptId, {
                error,
                status: "failed",
              });
            }
            const state = result.dashboard.script_updates[scriptId];
            return state
              ? applyCheckResult(next, scriptId, state)
              : updateRow(next, scriptId, {
                  error: "The runner did not return an update state.",
                  status: "failed",
                });
          }, current),
        );
      } catch (error) {
        setRows((current) =>
          scriptIds.reduce(
            (next, scriptId) =>
              updateRow(next, scriptId, {
                error: String(error),
                status: "failed",
              }),
            current,
          ),
        );
      }
    },
    () => false,
  );
}

async function prepareAvailableUpdates(
  scriptIds: string[],
  setRows: React.Dispatch<React.SetStateAction<CheckRow[]>>,
  setReviewQueue: React.Dispatch<React.SetStateAction<string[]>>,
  cancelled: React.RefObject<boolean>,
) {
  await runWithConcurrency(
    scriptIds,
    downloadConcurrency,
    async (scriptId) => {
        const requestId = crypto.randomUUID();
        setRows((current) => updateRow(current, scriptId, {
          error: null,
          preparation: "preparing",
          preparationProgress: null,
          preparationRequestId: requestId,
          review: null,
        }));
        try {
          const review = await prepareDiscoveredScriptUpdate(scriptId, requestId);
          setRows((current) => updateRow(current, scriptId, {
            preparation: "ready",
            preparationProgress: null,
            preparationRequestId: null,
            review,
          }));
          setReviewQueue((current) => current.includes(scriptId) ? current : [...current, scriptId]);
        } catch (error) {
          const message = String(error);
          setRows((current) => updateRow(current, scriptId, {
            error: isCancellation(message) ? null : message,
            preparation: isCancellation(message) ? "idle" : "failed",
            preparationProgress: null,
            preparationRequestId: null,
          }));
        }
    },
    () => cancelled.current,
  );
}

async function checkOne(
  script: ScriptStatus,
  onDashboard: (dashboard: DashboardPayload) => void,
) {
  const result = await checkScriptUpdate(script.installed.id);
  onDashboard(result.dashboard);
  return result.dashboard.script_updates[script.installed.id];
}

function applyCheckResult(rows: CheckRow[], scriptId: string, state: ScriptUpdateState) {
  return updateRow(rows, scriptId, {
    error: state.last_error,
    latestVersion: state.latest_version,
    status: state.status === "available" ? "available" : "current",
  });
}

function updateRow(rows: CheckRow[], scriptId: string, update: Partial<CheckRow>) {
  return rows.map((row) =>
    row.script.installed.id === scriptId ? { ...row, ...update } : row,
  );
}

function CheckStatusBadge({ status }: { status: CheckStatus }) {
  if (status === "available") return <Badge variant="medium">Update available</Badge>;
  if (status === "checking") return <Badge variant="muted">Checking</Badge>;
  if (status === "current") return <Badge variant="good"><CheckCircle2 />Up to date</Badge>;
  if (status === "failed") return <Badge variant="destructive"><AlertCircle />Check failed</Badge>;
  if (status === "pending") return <Badge variant="muted">Pending</Badge>;
  if (status === "preparing") return <Badge variant="muted">Preparing package</Badge>;
  if (status === "ready") return <Badge variant="medium">Ready for review</Badge>;
  if (status === "updated") return <Badge variant="good"><CheckCircle2 />Updated</Badge>;
  return <Badge variant="muted">Not configured</Badge>;
}

function preparationPercent(progress: RemotePreparationProgress | null) {
  if (!progress?.total_bytes || progress.total_bytes <= 0) return null;
  return Math.min(100, Math.round((progress.transferred_bytes / progress.total_bytes) * 100));
}

function preparationSummary(progress: RemotePreparationProgress | null) {
  if (!progress) return "Starting secure download";
  const labels: Record<RemotePreparationProgress["stage"], string> = {
    awaiting_review: "Preparing review",
    downloading_package: "Downloading package",
    downloading_repository: "Downloading repository",
    validating_package: "Validating package",
    verifying_hash: "Verifying package hash",
  };
  const total = progress.total_bytes
    ? ` ${formatBytes(progress.transferred_bytes)} of ${formatBytes(progress.total_bytes)}`
    : ` ${formatBytes(progress.transferred_bytes)}`;
  return `${labels[progress.stage]}.${total}`;
}

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KiB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MiB`;
}

function isCancellation(message: string) {
  return message.toLowerCase().includes("cancelled");
}

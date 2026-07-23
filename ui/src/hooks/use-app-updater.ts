import { isTauri } from "@tauri-apps/api/core";
import { getBundleType, getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { useCallback, useEffect, useRef, useState } from "react";

import {
  initialUpdateProgress,
  reduceUpdateProgress,
  type UpdateProgress,
} from "@/lib/update-progress";
import {
  prepareForUpdate,
  recordUpdateCheck,
  shouldCheckForUpdate,
} from "@/lib/runner-api";
import {
  canInstallUpdateInApp,
  classifyDownloadFailure,
  installationTypeFromBundle,
  type AppInstallationType,
  type UpdateFailureOperation,
} from "@/lib/update-policy";

export type AppUpdatePhase =
  | "idle"
  | "checking"
  | "up_to_date"
  | "available"
  | "downloading"
  | "ready"
  | "error";

export type AppUpdateState = {
  currentVersion: string | null;
  error: string | null;
  failedOperation: UpdateFailureOperation | null;
  installationType: AppInstallationType;
  lastCheckedAt: number | null;
  phase: AppUpdatePhase;
  progress: UpdateProgress;
  releaseNotes: string | null;
  version: string | null;
};

const initialState: AppUpdateState = {
  currentVersion: null,
  error: null,
  failedOperation: null,
  installationType: "unknown",
  lastCheckedAt: null,
  phase: "idle",
  progress: initialUpdateProgress,
  releaseNotes: null,
  version: null,
};

export function useAppUpdater(onError: (message: string) => void, checkOnStartup: boolean) {
  const [state, setState] = useState(initialState);
  const [dialogOpen, setDialogOpen] = useState(false);
  const updateRef = useRef<Update | null>(null);
  const downloadedRef = useRef(false);
  const checkedOnStartup = useRef(false);
  const installationTypeRef = useRef<AppInstallationType | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    void getVersion()
      .then((version) => {
        setState((current) => ({
          ...current,
          currentVersion: current.currentVersion ?? version,
        }));
      })
      .catch((error) => onError(`Could not read the application version: ${String(error)}`));
  }, [onError]);

  const resolveInstallationType = useCallback(async () => {
    if (installationTypeRef.current) return installationTypeRef.current;
    try {
      const installationType = installationTypeFromBundle(await getBundleType());
      installationTypeRef.current = installationType;
      setState((current) => ({ ...current, installationType }));
      return installationType;
    } catch (error) {
      installationTypeRef.current = "unknown";
      setState((current) => ({ ...current, installationType: "unknown" }));
      onError(`Could not identify the installation type: ${String(error)}`);
      return "unknown" as const;
    }
  }, [onError]);

  useEffect(() => {
    if (!isTauri()) return;
    void resolveInstallationType();
  }, [resolveInstallationType]);

  const checkForUpdate = useCallback(async () => {
    if (!isTauri()) return;
    const installationType = await resolveInstallationType();
    setState((current) => ({
      ...current,
      error: null,
      failedOperation: null,
      installationType,
      phase: "checking",
    }));
    setDialogOpen(false);
    try {
      const previousUpdate = updateRef.current;
      updateRef.current = null;
      downloadedRef.current = false;
      await previousUpdate?.close();
      const update = await check({ timeout: 15_000 });
      await recordUpdateCheck(update?.version ?? null, update?.body?.trim() || null);
      if (!update) {
        setState((current) => ({
          ...initialState,
          currentVersion: current.currentVersion,
          installationType,
          lastCheckedAt: Date.now(),
          phase: "up_to_date",
        }));
        return;
      }
      updateRef.current = update;
      downloadedRef.current = false;
      setState({
        currentVersion: update.currentVersion,
        error: null,
        failedOperation: null,
        installationType,
        lastCheckedAt: Date.now(),
        phase: "available",
        progress: initialUpdateProgress,
        releaseNotes: update.body?.trim() || null,
        version: update.version,
      });
      setDialogOpen(true);
    } catch (error) {
      const message = `Unable to check for updates: ${String(error)}`;
      setState((current) => ({
        ...current,
        error: message,
        failedOperation: "check",
        installationType,
        phase: "error",
      }));
      setDialogOpen(true);
      onError(message);
    }
  }, [onError, resolveInstallationType]);

  useEffect(() => {
    if (!checkOnStartup || checkedOnStartup.current) return;
    checkedOnStartup.current = true;
    void shouldCheckForUpdate()
      .then((due) => {
        if (due) return checkForUpdate();
      })
      .catch((error) => onError(`Update schedule check failed: ${String(error)}`));
  }, [checkForUpdate, checkOnStartup, onError]);

  const download = useCallback(async () => {
    const update = updateRef.current;
    if (!update) return;
    if (!canInstallUpdateInApp(state.installationType)) {
      onError("This installation is updated through its Linux package manager.");
      return;
    }
    setState((current) => ({
      ...current,
      error: null,
      failedOperation: null,
      phase: "downloading",
      progress: initialUpdateProgress,
    }));
    setDialogOpen(true);
    try {
      await update.download((event) => {
        setState((current) => ({
          ...current,
          progress: reduceUpdateProgress(current.progress, event),
        }));
      });
      downloadedRef.current = true;
      setState((current) => ({ ...current, phase: "ready" }));
    } catch (error) {
      const failedOperation = classifyDownloadFailure(error);
      const message = failedOperation === "verify"
        ? `Update verification failed: ${String(error)}`
        : `Update download failed: ${String(error)}`;
      setState((current) => ({
        ...current,
        error: message,
        failedOperation,
        phase: "error",
      }));
      setDialogOpen(true);
      onError(message);
    }
  }, [onError, state.installationType]);

  const installAndRestart = useCallback(async () => {
    const update = updateRef.current;
    if (!update) return;
    if (!canInstallUpdateInApp(state.installationType)) {
      onError("This installation is updated through its Linux package manager.");
      return;
    }
    try {
      await prepareForUpdate();
      await update.install();
      await relaunch();
    } catch (error) {
      const message = `Update installation failed: ${String(error)}`;
      setState((current) => ({
        ...current,
        error: message,
        failedOperation: "install",
        phase: "error",
      }));
      setDialogOpen(true);
      onError(message);
    }
  }, [onError, state.installationType]);

  const retry = useCallback(() => {
    setDialogOpen(true);
    if (state.failedOperation === "install" && downloadedRef.current) {
      return installAndRestart();
    }
    if (
      (state.failedOperation === "download" || state.failedOperation === "verify") &&
      updateRef.current
    ) {
      return download();
    }
    return checkForUpdate();
  }, [checkForUpdate, download, installAndRestart, state.failedOperation]);

  const dismiss = useCallback(() => {
    if (state.phase === "downloading" || state.phase === "ready") return;
    setDialogOpen(false);
  }, [state.phase]);

  return {
    checkForUpdate,
    dialogOpen,
    dismiss,
    download,
    installAndRestart,
    retry,
    state,
  };
}

export type AppUpdaterController = ReturnType<typeof useAppUpdater>;

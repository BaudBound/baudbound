import { isTauri } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
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
  lastCheckedAt: number | null;
  phase: AppUpdatePhase;
  progress: UpdateProgress;
  releaseNotes: string | null;
  version: string | null;
};

const initialState: AppUpdateState = {
  currentVersion: null,
  error: null,
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

  const checkForUpdate = useCallback(async () => {
    if (!isTauri()) return;
    setState((current) => ({ ...current, error: null, phase: "checking" }));
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
        lastCheckedAt: Date.now(),
        phase: "available",
        progress: initialUpdateProgress,
        releaseNotes: update.body?.trim() || null,
        version: update.version,
      });
      setDialogOpen(true);
    } catch (error) {
      const message = `Update check failed: ${String(error)}`;
      setState((current) => ({ ...current, error: message, phase: "error" }));
      setDialogOpen(true);
      onError(message);
    }
  }, [onError]);

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
    setState((current) => ({
      ...current,
      error: null,
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
      const message = `Update failed: ${String(error)}`;
      setState((current) => ({ ...current, error: message, phase: "error" }));
      setDialogOpen(true);
      onError(message);
    }
  }, [onError]);

  const installAndRestart = useCallback(async () => {
    const update = updateRef.current;
    if (!update) return;
    try {
      await prepareForUpdate();
      await update.install();
      await relaunch();
    } catch (error) {
      const message = `Update installation failed: ${String(error)}`;
      setState((current) => ({ ...current, error: message, phase: "error" }));
      setDialogOpen(true);
      onError(message);
    }
  }, [onError]);

  const retry = useCallback(() => {
    setDialogOpen(true);
    if (downloadedRef.current) {
      return installAndRestart();
    }
    if (updateRef.current) {
      return download();
    }
    return checkForUpdate();
  }, [checkForUpdate, download, installAndRestart]);

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

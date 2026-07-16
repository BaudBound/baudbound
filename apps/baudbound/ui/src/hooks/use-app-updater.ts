import { isTauri } from "@tauri-apps/api/core";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { useCallback, useEffect, useRef, useState } from "react";

import {
  initialUpdateProgress,
  reduceUpdateProgress,
  type UpdateProgress,
} from "@/lib/update-progress";
import { prepareForUpdate } from "@/lib/runner-api";

export type AppUpdatePhase = "idle" | "checking" | "available" | "downloading" | "ready" | "error";

export type AppUpdateState = {
  currentVersion: string | null;
  error: string | null;
  phase: AppUpdatePhase;
  progress: UpdateProgress;
  releaseNotes: string | null;
  version: string | null;
};

const initialState: AppUpdateState = {
  currentVersion: null,
  error: null,
  phase: "idle",
  progress: initialUpdateProgress,
  releaseNotes: null,
  version: null,
};

export function useAppUpdater(onError: (message: string) => void, checkOnStartup: boolean) {
  const [state, setState] = useState(initialState);
  const updateRef = useRef<Update | null>(null);
  const downloadedRef = useRef(false);
  const checkedOnStartup = useRef(false);

  const checkForUpdate = useCallback(async () => {
    if (!isTauri()) return;
    setState((current) => ({ ...current, error: null, phase: "checking" }));
    try {
      const update = await check({ timeout: 15_000 });
      if (!update) {
        setState(initialState);
        return;
      }
      updateRef.current = update;
      downloadedRef.current = false;
      setState({
        currentVersion: update.currentVersion,
        error: null,
        phase: "available",
        progress: initialUpdateProgress,
        releaseNotes: update.body?.trim() || null,
        version: update.version,
      });
    } catch (error) {
      const message = `Update check failed: ${String(error)}`;
      setState((current) => ({ ...current, error: message, phase: "error" }));
      onError(message);
    }
  }, [onError]);

  useEffect(() => {
    if (!checkOnStartup || checkedOnStartup.current) return;
    checkedOnStartup.current = true;
    void checkForUpdate();
  }, [checkForUpdate, checkOnStartup]);

  const download = useCallback(async () => {
    const update = updateRef.current;
    if (!update) return;
    setState((current) => ({
      ...current,
      error: null,
      phase: "downloading",
      progress: initialUpdateProgress,
    }));
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
      onError(message);
    }
  }, [onError]);

  const retry = useCallback(() => {
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
    void updateRef.current?.close();
    updateRef.current = null;
    downloadedRef.current = false;
    setState(initialState);
  }, [state.phase]);

  return {
    checkForUpdate,
    dismiss,
    download,
    installAndRestart,
    retry,
    state,
  };
}

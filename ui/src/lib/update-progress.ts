import type { DownloadEvent } from "@tauri-apps/plugin-updater";

export type UpdateProgress = {
  downloadedBytes: number;
  totalBytes: number | null;
};

export const initialUpdateProgress: UpdateProgress = {
  downloadedBytes: 0,
  totalBytes: null,
};

export function reduceUpdateProgress(
  progress: UpdateProgress,
  event: DownloadEvent,
): UpdateProgress {
  if (event.event === "Started") {
    return {
      downloadedBytes: 0,
      totalBytes: positiveNumberOrNull(event.data.contentLength),
    };
  }
  if (event.event === "Progress") {
    return {
      ...progress,
      downloadedBytes: progress.downloadedBytes + Math.max(0, event.data.chunkLength),
    };
  }
  return {
    downloadedBytes: progress.totalBytes ?? progress.downloadedBytes,
    totalBytes: progress.totalBytes,
  };
}

export function updateProgressPercent(progress: UpdateProgress): number | null {
  if (!progress.totalBytes) return null;
  return Math.min(100, Math.round((progress.downloadedBytes / progress.totalBytes) * 100));
}

function positiveNumberOrNull(value: number | undefined): number | null {
  return typeof value === "number" && value > 0 ? value : null;
}

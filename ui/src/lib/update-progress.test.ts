import { describe, expect, it } from "vitest";

import {
  initialUpdateProgress,
  reduceUpdateProgress,
  updateProgressPercent,
} from "@/lib/update-progress";

describe("update progress", () => {
  it("tracks bounded progress from Tauri download events", () => {
    const started = reduceUpdateProgress(initialUpdateProgress, {
      event: "Started",
      data: { contentLength: 100 },
    });
    const halfway = reduceUpdateProgress(started, {
      event: "Progress",
      data: { chunkLength: 50 },
    });
    const overReported = reduceUpdateProgress(halfway, {
      event: "Progress",
      data: { chunkLength: 75 },
    });

    expect(updateProgressPercent(halfway)).toBe(50);
    expect(updateProgressPercent(overReported)).toBe(100);
  });

  it("supports downloads without a content length", () => {
    const started = reduceUpdateProgress(initialUpdateProgress, {
      event: "Started",
      data: { contentLength: 0 },
    });
    const progressed = reduceUpdateProgress(started, {
      event: "Progress",
      data: { chunkLength: 512 },
    });

    expect(progressed.downloadedBytes).toBe(512);
    expect(updateProgressPercent(progressed)).toBeNull();
  });
});

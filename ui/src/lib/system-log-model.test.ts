import { describe, expect, it } from "vitest";

import { createSystemLog, formatSystemLogDetails } from "@/lib/system-log-model";

describe("system log model", () => {
  it("captures complete Error diagnostics", () => {
    const cause = new Error("socket closed");
    const error = new Error("request failed", { cause });
    const log = createSystemLog("error", "The request could not be completed.", {
      error,
      source: "Network",
      title: "Request failed",
    });

    expect(log.details).toContainEqual({
      label: "Error message",
      value: "request failed",
    });
    expect(log.details.find((detail) => detail.label === "Error cause")?.value).toContain(
      "socket closed",
    );
    expect(log.details.some((detail) => detail.label === "Stack trace")).toBe(true);
  });

  it("removes null characters and bounds oversized values", () => {
    const log = createSystemLog("info", `message\0${"x".repeat(10_000)}`, {
      details: [{ label: "Payload", value: "y".repeat(40_000) }],
      source: "Test\0 source",
    });

    expect(log.message).not.toContain("\0");
    expect(new TextEncoder().encode(log.message).length).toBeLessThanOrEqual(8 * 1024);
    expect(new TextEncoder().encode(log.details[0].value).length).toBeLessThanOrEqual(
      32 * 1024,
    );
  });

  it("formats every field for copying", () => {
    const text = formatSystemLogDetails({
      details: [{ label: "Operation", value: "Reload" }],
      message: "Runner reloaded.",
      severity: "success",
      source: "Service",
      timestamp_unix_ms: 1_000,
      title: "Reload complete",
    });

    expect(text).toContain("Timestamp: 1970-01-01T00:00:01.000Z");
    expect(text).toContain("Operation: Reload");
    expect(text).toContain("Message: Runner reloaded.");
  });
});

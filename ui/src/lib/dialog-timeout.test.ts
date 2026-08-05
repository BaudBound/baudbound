import { describe, expect, it } from "vitest";

import { formatDialogTimeout, remainingDialogTimeoutMs } from "@/lib/dialog-timeout";

describe("desktop dialog timeout", () => {
  it("counts down from the absolute deadline without becoming negative", () => {
    expect(remainingDialogTimeoutMs(12_500, 10_000)).toBe(2_500);
    expect(remainingDialogTimeoutMs(12_500, 13_000)).toBe(0);
  });

  it("formats seconds, minutes, and hours without dropping partial seconds", () => {
    expect(formatDialogTimeout(2_001)).toBe("3s");
    expect(formatDialogTimeout(65_000)).toBe("1:05");
    expect(formatDialogTimeout(3_665_000)).toBe("1:01:05");
  });
});

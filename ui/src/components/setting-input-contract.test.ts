import { describe, expect, it } from "vitest";

import { isValidColor, rgbaToHex } from "@/components/color-value-input";
import { validateHotkey } from "@/components/hotkey-input";

describe("Script Setting input contracts", () => {
  it("accepts canonical colors and rejects arbitrary strings", () => {
    expect(isValidColor("#1A2b3C")).toBe(true);
    expect(isValidColor("#12345G")).toBe(false);
    expect(isValidColor("red")).toBe(false);
  });

  it("serializes picker RGB channels as canonical uppercase hex", () => {
    expect(rgbaToHex([188, 38, 1, 1])).toBe("#BC2601");
    expect(rgbaToHex([0.4, 15.6, 255, 1])).toBe("#0010FF");
  });

  it("accepts supported Windows chords and rejects unknown or duplicate keys", () => {
    expect(validateHotkey("Ctrl+Shift+F8")).toBe(true);
    expect(validateHotkey("Ctrl+DefinitelyNotAKey")).toBe(false);
    expect(validateHotkey("Ctrl+Ctrl+F8")).toBe(false);
  });
});

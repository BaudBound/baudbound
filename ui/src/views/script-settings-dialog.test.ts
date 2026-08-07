import { describe, expect, it } from "vitest";

import type { ScriptSettingValueType } from "@/lib/runner-api";
import { validateDraftValue } from "@/views/script-settings-dialog";

function check(valueType: ScriptSettingValueType, value: string) {
  return validateDraftValue(valueType, null, value);
}

describe("script setting drafts", () => {
  it("validates a keyboard key as a key expression rather than as JSON", () => {
    // The dialog spoke the previous vocabulary, so a keyboard key setting fell
    // through to the JSON branch. It offered no key capture and reported
    // "Enter valid JSON." for a key that was simply not recognised.
    expect(check("hotkey", "Ctrl+Shift+F8")).toBeNull();
    expect(check("hotkey", "Ctrl+NotARealKey")).toMatch(/Windows key combination/);
    expect(check("hotkey", "Ctrl+NotARealKey")).not.toMatch(/JSON/);
  });

  it("keeps integer and float apart", () => {
    expect(check("integer", "12")).toBeNull();
    expect(check("integer", "12.5")).toMatch(/whole number/);
    expect(check("float", "12.5")).toBeNull();
    expect(check("float", "abc")).toMatch(/finite number/);
  });

  it("reports every remaining type in its own terms", () => {
    expect(check("string", "anything")).toBeNull();
    expect(check("color", "#1A2B3C")).toBeNull();
    expect(check("color", "red")).toMatch(/#RRGGBB/);
    expect(check("boolean", "true")).toBeNull();
    expect(check("boolean", "yes")).toMatch(/true or false/);
    expect(check("object", '{"a":1}')).toBeNull();
    expect(check("datetime", '{"type":"datetime","value":"2026-01-01T00:00:00.000Z"}')).toBeNull();
    expect(check("duration", '{"type":"duration","unit":"seconds","value":3}')).toBeNull();
  });
});

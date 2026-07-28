import { describe, expect, it } from "vitest";

import {
  evaluatePasswordStrength,
  passwordCharacterCount,
} from "@/lib/password-strength";

describe("password strength", () => {
  it("counts Unicode characters instead of UTF-16 code units", () => {
    expect(passwordCharacterCount("1234567😀")).toBe(8);
  });

  it("rejects passwords shorter than eight characters", () => {
    expect(evaluatePasswordStrength("Ab1!xyz")).toEqual({
      label: "Too short",
      score: 0,
    });
  });

  it("keeps common passwords weak", () => {
    expect(evaluatePasswordStrength("Password123!").label).toBe("Weak");
  });

  it("recognizes long varied passwords as strong", () => {
    expect(evaluatePasswordStrength("River-Glass-84!North")).toEqual({
      label: "Strong",
      score: 4,
    });
  });
});

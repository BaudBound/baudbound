import { describe, expect, it } from "vitest";

import { visibleText } from "@/lib/visible-text";

describe("visibleText", () => {
  it("keeps ordinary quotes readable while exposing control characters", () => {
    expect(visibleText('Scale data: "\fN: 348g\r\n"')).toBe(
      'Scale data: "\\fN: 348g\\r\\n"',
    );
  });

  it("distinguishes literal backslashes from escaped control characters", () => {
    expect(visibleText(String.raw`literal \n`)).toBe(String.raw`literal \\n`);
  });
});

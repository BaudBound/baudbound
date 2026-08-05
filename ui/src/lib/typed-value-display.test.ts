import { describe, expect, it } from "vitest";

import { formatTypedValueForDisplay } from "@/lib/typed-value-display";

describe("typed value display", () => {
  it("shows datetime data without its runtime wrapper", () => {
    expect(
      formatTypedValueForDisplay("datetime", {
        type: "datetime",
        value: "2026-08-04T12:34:56.000Z",
      }),
    ).toBe("2026-08-04T12:34:56.000Z");
  });

  it("shows duration data with a human-readable unit", () => {
    expect(formatTypedValueForDisplay("duration", { type: "duration", unit: "seconds", value: 5 })).toBe(
      "5 seconds",
    );
    expect(formatTypedValueForDisplay("duration", { type: "duration", unit: "hours", value: 1 })).toBe("1 hour");
  });

  it("formats typed list items without JSON container syntax", () => {
    expect(
      formatTypedValueForDisplay(
        "list",
        [
          { type: "duration", unit: "minutes", value: 1 },
          { type: "duration", unit: "minutes", value: 3 },
        ],
        "duration",
      ),
    ).toBe("1 minute\n3 minutes");
  });

  it("retains JSON formatting only for arbitrary object values", () => {
    expect(formatTypedValueForDisplay("object", { enabled: true })).toBe('{\n  "enabled": true\n}');
  });

  it("does not expose malformed custom wrappers as JSON", () => {
    expect(formatTypedValueForDisplay("datetime", { type: "datetime", value: "not-a-date" })).toBe(
      "Invalid date and time",
    );
    expect(formatTypedValueForDisplay("duration", { type: "duration", unit: "weeks", value: 2 })).toBe(
      "Invalid duration",
    );
  });
});

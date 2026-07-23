/// <reference types="vite/client" />

import { describe, expect, it } from "vitest";

import { createDesktopTimeFormatter } from "@/lib/time-format";

const sample = new Date(Date.UTC(2026, 6, 17, 13, 5, 9));

describe("desktop time formatting", () => {
  it("formats the same timestamp with explicit 12-hour and 24-hour clocks", () => {
    const twelveHour = createDesktopTimeFormatter("12-hour", {
      locale: "en-US",
      timeZone: "UTC",
    });
    const twentyFourHour = createDesktopTimeFormatter("24-hour", {
      locale: "en-US",
      timeZone: "UTC",
    });

    expect(twelveHour.formatTime(sample)).toContain("1:05:09 PM");
    expect(twentyFourHour.formatTime(sample)).toContain("13:05:09");
    expect(twentyFourHour.formatTime(sample)).not.toContain("PM");
  });

  it("keeps Unix second and millisecond inputs on the same instant", () => {
    const formatter = createDesktopTimeFormatter("24-hour", {
      locale: "en-US",
      timeZone: "UTC",
    });

    expect(formatter.formatUnixSeconds(sample.getTime() / 1_000)).toBe(
      formatter.formatUnixMilliseconds(sample.getTime()),
    );
  });

  it("keeps desktop timestamp presentation behind the shared formatter", () => {
    const sourceFiles = import.meta.glob("../**/*.{ts,tsx}", {
      eager: true,
      import: "default",
      query: "?raw",
    }) as Record<string, string>;

    for (const [file, source] of Object.entries(sourceFiles)) {
      if (file.endsWith("time-format.tsx")) {
        continue;
      }
      expect(source, file).not.toMatch(/\.toLocale(?:Date|Time)?String\s*\(/);
      expect(source, file).not.toMatch(/new\s+Intl\.DateTimeFormat\s*\(/);
    }
  });
});

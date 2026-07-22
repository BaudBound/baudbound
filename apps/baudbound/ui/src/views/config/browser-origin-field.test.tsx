import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import {
  BROWSER_ORIGIN_MAX_COUNT,
  BROWSER_ORIGIN_MAX_LENGTH,
} from "@/lib/input-limits";
import {
  appendBrowserOrigins,
  BrowserOriginField,
  isValidBrowserOrigin,
  parseBrowserOrigins,
} from "@/views/config/browser-origin-field";

describe("browser origin field", () => {
  it("renders configured origins as wrapping removable badges", () => {
    const markup = renderToStaticMarkup(
      <BrowserOriginField
        onChange={() => undefined}
        value={["https://dashboard.example.com", "http://localhost:3000"]}
      />,
    );

    expect(markup).toContain("flex-wrap");
    expect(markup).toContain("https://dashboard.example.com");
    expect(markup).toContain("Remove http://localhost:3000");
  });

  it("parses comma, space, and line separated origins", () => {
    expect(
      parseBrowserOrigins(
        "https://one.example, http://localhost:3000\nhttps://two.example",
      ),
    ).toEqual([
      "https://one.example",
      "http://localhost:3000",
      "https://two.example",
    ]);
  });

  it("accepts exact origins and rejects paths, credentials, and unsupported schemes", () => {
    expect(isValidBrowserOrigin("https://dashboard.example.com")).toBe(true);
    expect(isValidBrowserOrigin("http://localhost:3000")).toBe(true);
    expect(isValidBrowserOrigin("https://dashboard.example.com/path")).toBe(false);
    expect(isValidBrowserOrigin("https://user@example.com")).toBe(false);
    expect(isValidBrowserOrigin("wss://dashboard.example.com")).toBe(false);
  });

  it("adds valid origins without duplicates and preserves input on an error", () => {
    const current = ["https://one.example"];
    expect(
      appendBrowserOrigins(
        current,
        "https://one.example, https://two.example",
      ),
    ).toEqual({
      origins: ["https://one.example", "https://two.example"],
      error: null,
    });

    expect(appendBrowserOrigins(current, "https://invalid.example/path")).toEqual({
      origins: current,
      error: "https://invalid.example/path is not an exact http or https origin.",
    });
  });

  it("rejects oversized origins and origin lists", () => {
    expect(isValidBrowserOrigin(`https://${"a".repeat(BROWSER_ORIGIN_MAX_LENGTH)}`)).toBe(false);
    const origins = Array.from(
      { length: BROWSER_ORIGIN_MAX_COUNT },
      (_, index) => `https://${index}.example.com`,
    );

    expect(appendBrowserOrigins(origins, "https://extra.example.com").error).toContain(
      "No more than",
    );
  });
});

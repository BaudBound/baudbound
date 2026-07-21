import { describe, expect, it } from "vitest";

import { normalizeExternalUrl } from "@/lib/external-url";

describe("normalizeExternalUrl", () => {
  it("accepts HTTP and HTTPS project links", () => {
    expect(normalizeExternalUrl("https://baudbound.app/docs")).toBe(
      "https://baudbound.app/docs",
    );
    expect(normalizeExternalUrl("http://localhost:3000")).toBe("http://localhost:3000/");
  });

  it("rejects executable and local URL schemes", () => {
    expect(() => normalizeExternalUrl("javascript:alert(1)")).toThrow(
      "Only HTTP and HTTPS links can be opened.",
    );
    expect(() => normalizeExternalUrl("file:///tmp/package.bbs")).toThrow(
      "Only HTTP and HTTPS links can be opened.",
    );
  });
});

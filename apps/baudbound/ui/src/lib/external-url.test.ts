import { describe, expect, it } from "vitest";

import { normalizeExternalUrl, tryNormalizeExternalUrl } from "@/lib/external-url";

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

  it("returns no renderable URL for invalid or unsafe values", () => {
    expect(tryNormalizeExternalUrl("javascript:alert(1)")).toBeNull();
    expect(tryNormalizeExternalUrl("not a URL")).toBeNull();
    expect(tryNormalizeExternalUrl(" https://baudbound.app/docs ")).toBe(
      "https://baudbound.app/docs",
    );
  });
});

import { describe, expect, it } from "vitest";

import { formatCount } from "@/lib/count-format";

describe("count formatting", () => {
  it("uses the singular noun for one item", () => {
    expect(formatCount(1, "script")).toBe("1 script");
  });

  it("uses the plural noun for other counts", () => {
    expect(formatCount(0, "script")).toBe("0 scripts");
    expect(formatCount(2, "script")).toBe("2 scripts");
  });
});

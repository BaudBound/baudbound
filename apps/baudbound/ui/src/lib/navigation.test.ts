import { describe, expect, it } from "vitest";

import { navigationGroups, navigationItems, pageTitle } from "@/lib/navigation";

describe("runner navigation", () => {
  it("exposes Tools in System without the removed Devices destination", () => {
    const system = navigationGroups.find((group) => group.label === "System");

    expect(system?.items.map((item) => item.id)).toEqual(["tools", "config", "diagnostics"]);
    expect(navigationItems.some((item) => item.id === "tools")).toBe(true);
    expect(navigationItems.some((item) => String(item.id) === "devices")).toBe(false);
  });

  it("uses the Tools page identity", () => {
    expect(pageTitle("tools")).toBe("Tools");
  });
});

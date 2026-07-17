import { describe, expect, it } from "vitest";

import { installContextMenuGuard } from "@/lib/context-menu";

describe("desktop context menu guard", () => {
  it("prevents context menus until its cleanup runs", () => {
    const target = new EventTarget();
    const cleanup = installContextMenuGuard(target);
    const blocked = new Event("contextmenu", { cancelable: true });

    expect(target.dispatchEvent(blocked)).toBe(false);
    expect(blocked.defaultPrevented).toBe(true);

    cleanup();
    const allowed = new Event("contextmenu", { cancelable: true });
    expect(target.dispatchEvent(allowed)).toBe(true);
    expect(allowed.defaultPrevented).toBe(false);
  });
});

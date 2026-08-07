import { describe, expect, it } from "vitest";

import { isBrowserChromeShortcut, isReloadShortcut } from "@/lib/browser-chrome";

function press(key: string, modifiers: Partial<KeyboardEvent> = {}) {
  return { altKey: false, ctrlKey: false, metaKey: false, shiftKey: false, key, ...modifiers } as KeyboardEvent;
}

describe("browser chrome shortcuts", () => {
  it("blocks the browser chrome the runner has no use for", () => {
    for (const key of ["f", "g", "p", "r", "u", "s", "o", "d", "h", "j", "n", "t"]) {
      expect(isBrowserChromeShortcut(press(key, { ctrlKey: true }), false), key).toBe(true);
    }
    for (const key of ["F3", "F5", "F11"]) {
      expect(isBrowserChromeShortcut(press(key), false), key).toBe(true);
    }
    expect(isBrowserChromeShortcut(press("ArrowLeft", { altKey: true }), false)).toBe(true);
  });

  it("blocks page zoom", () => {
    for (const key of ["+", "=", "-", "_", "0"]) {
      expect(isBrowserChromeShortcut(press(key, { ctrlKey: true }), false), key).toBe(true);
    }
  });

  it("leaves the standard editing shortcuts alone", () => {
    // Blocking these would break text entry rather than make the runner feel
    // native, so they stay whatever else is disabled.
    for (const key of ["c", "v", "x", "a", "z", "y"]) {
      expect(isBrowserChromeShortcut(press(key, { ctrlKey: true }), false), key).toBe(false);
    }
    for (const key of ["Tab", "Home", "End", "ArrowLeft", "Enter", "Backspace", "a"]) {
      expect(isBrowserChromeShortcut(press(key), false), key).toBe(false);
    }
  });

  it("keeps the developer tools reachable while developing", () => {
    expect(isBrowserChromeShortcut(press("F12"), true)).toBe(false);
    expect(isBrowserChromeShortcut(press("i", { ctrlKey: true, shiftKey: true }), true)).toBe(false);

    expect(isBrowserChromeShortcut(press("F12"), false)).toBe(true);
    expect(isBrowserChromeShortcut(press("i", { ctrlKey: true, shiftKey: true }), false)).toBe(true);
  });

  it("treats the command key like the control key", () => {
    expect(isBrowserChromeShortcut(press("f", { metaKey: true }), false)).toBe(true);
    expect(isBrowserChromeShortcut(press("c", { metaKey: true }), false)).toBe(false);
  });
});

describe("reload", () => {
  it("keeps the modified reload and refuses the bare one", () => {
    // The window reload is the one browser behaviour worth keeping, and the
    // guard carries it out itself because WebView2 refuses the accelerator.
    expect(isReloadShortcut(press("F5", { ctrlKey: true }))).toBe(true);
    expect(isReloadShortcut(press("F5", { metaKey: true }))).toBe(true);
    expect(isBrowserChromeShortcut(press("F5", { ctrlKey: true }), false)).toBe(false);

    // A stray F5 still does nothing, so a run is not restarted by accident.
    expect(isReloadShortcut(press("F5"))).toBe(false);
    expect(isBrowserChromeShortcut(press("F5"), false)).toBe(true);
  });
});

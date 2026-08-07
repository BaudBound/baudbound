/**
 * Suppresses the browser shortcuts that have no meaning in the runner.
 *
 * WebView2 can switch most of these off through its own settings, which is the
 * authoritative fix on Windows. This covers the platforms that cannot, and the
 * cases the setting does not reach, such as zooming with the wheel.
 *
 * Standard editing shortcuts are deliberately untouched. Copy, paste, cut,
 * undo, redo, select all and tab navigation all keep working, because blocking
 * them would break text entry and keyboard accessibility rather than make the
 * runner feel native.
 */

type ChromeShortcut = {
  key: string;
  shift?: boolean;
};

/// Matched with the control or command modifier held.
const modifiedShortcuts: ChromeShortcut[] = [
  { key: "f" }, // find bar
  { key: "g" }, // find next
  { key: "g", shift: true }, // find previous
  { key: "p" }, // print
  { key: "r" }, // reload
  { key: "r", shift: true }, // reload ignoring cache
  { key: "u" }, // view source
  { key: "s" }, // save page
  { key: "o" }, // open file
  { key: "d" }, // bookmark
  { key: "h" }, // history
  { key: "j" }, // downloads
  { key: "n" }, // new window
  { key: "t" }, // new tab
  { key: "+" },
  { key: "=" },
  { key: "-" },
  { key: "_" },
  { key: "0" },
];

/// Matched on their own, with no modifier required.
const bareShortcuts = new Set(["F3", "F5", "F7"]);

/**
 * Reloads the window, the one browser behaviour worth keeping.
 *
 * WebView2 refuses the browser accelerators outright, so the shortcut cannot
 * be let through and has to be carried out here instead. Only the modified
 * form reloads, so a stray F5 still does nothing.
 */
export function isReloadShortcut(event: KeyboardEvent) {
  return event.key === "F5" && (event.ctrlKey || event.metaKey);
}

/// Developer tools, kept available while developing the runner.
const developerShortcuts: ChromeShortcut[] = [
  { key: "i", shift: true },
  { key: "j", shift: true },
  { key: "c", shift: true },
];

export function isBrowserChromeShortcut(event: KeyboardEvent, allowDeveloperTools: boolean) {
  if (isReloadShortcut(event)) {
    return false;
  }
  if (bareShortcuts.has(event.key)) {
    return true;
  }
  if (event.key === "F12" || event.key === "F11") {
    // F11 is full screen, which leaves the runner without its own frame.
    return event.key === "F11" || !allowDeveloperTools;
  }
  if (event.altKey && (event.key === "ArrowLeft" || event.key === "ArrowRight")) {
    // Back and forward, which would leave the interface with nowhere to go.
    return true;
  }

  const modifier = event.ctrlKey || event.metaKey;
  if (!modifier) {
    return false;
  }

  const key = event.key.toLowerCase();
  if (!allowDeveloperTools && developerShortcuts.some((shortcut) => matches(shortcut, key, event))) {
    return true;
  }
  return modifiedShortcuts.some((shortcut) => matches(shortcut, key, event));
}

function matches(shortcut: ChromeShortcut, key: string, event: KeyboardEvent) {
  return shortcut.key === key && (shortcut.shift ?? false) === event.shiftKey;
}

export function installBrowserChromeGuard(
  target: EventTarget,
  options: { allowDeveloperTools: boolean },
) {
  const capture = { capture: true } as const;
  const onKeyDown = (event: Event) => {
    if (!(event instanceof KeyboardEvent)) return;
    if (isReloadShortcut(event)) {
      event.preventDefault();
      window.location.reload();
      return;
    }
    if (isBrowserChromeShortcut(event, options.allowDeveloperTools)) {
      event.preventDefault();
    }
  };
  // Zooming with the wheel needs a non passive listener to be refusable.
  const onWheel = (event: Event) => {
    if (event instanceof WheelEvent && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
    }
  };

  target.addEventListener("keydown", onKeyDown, capture);
  target.addEventListener("wheel", onWheel, { capture: true, passive: false });
  return () => {
    target.removeEventListener("keydown", onKeyDown, capture);
    target.removeEventListener("wheel", onWheel, capture);
  };
}

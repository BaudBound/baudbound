import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "@/app";
import { installContextMenuGuard } from "@/lib/context-menu";
import "@/styles.css";
import { CoordinatePickerOverlay } from "@/views/tools/coordinate-picker-overlay";
import { DesktopDialogConsoleView, DesktopDialogView } from "@/views/desktop-dialog-view";

const coordinatePickerSession = new URLSearchParams(window.location.search).get(
  "coordinatePicker",
);
const desktopDialogRequest = new URLSearchParams(window.location.search).get(
  "desktopDialog",
);
const desktopDialogConsole = new URLSearchParams(window.location.search).get(
  "desktopDialogConsole",
);
if (coordinatePickerSession) {
  document.documentElement.classList.add("coordinate-picker-document");
} else if (desktopDialogRequest || desktopDialogConsole) {
  document.documentElement.classList.add("desktop-dialog-document");
}

installContextMenuGuard(window);

createRoot(document.getElementById("root") as HTMLElement).render(
  <StrictMode>
    {coordinatePickerSession ? (
      <CoordinatePickerOverlay sessionId={coordinatePickerSession} />
    ) : desktopDialogRequest ? (
      <DesktopDialogView requestId={desktopDialogRequest} />
    ) : desktopDialogConsole ? (
      <DesktopDialogConsoleView />
    ) : (
      <App />
    )}
  </StrictMode>,
);

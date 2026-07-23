import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "@/app";
import { installContextMenuGuard } from "@/lib/context-menu";
import "@/styles.css";
import { CoordinatePickerOverlay } from "@/views/tools/coordinate-picker-overlay";

const coordinatePickerSession = new URLSearchParams(window.location.search).get(
  "coordinatePicker",
);
if (coordinatePickerSession) {
  document.documentElement.classList.add("coordinate-picker-document");
}

installContextMenuGuard(window);

createRoot(document.getElementById("root") as HTMLElement).render(
  <StrictMode>
    {coordinatePickerSession ? (
      <CoordinatePickerOverlay sessionId={coordinatePickerSession} />
    ) : (
      <App />
    )}
  </StrictMode>,
);

import { Crosshair } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import { cancelCoordinatePicker, selectCoordinatePicker } from "@/lib/runner-api";

export function CoordinatePickerOverlay({ sessionId }: { sessionId: string }) {
  const [isFinishing, setIsFinishing] = useState(false);

  const cancel = useCallback(() => {
    if (isFinishing) return;
    setIsFinishing(true);
    void cancelCoordinatePicker(sessionId);
  }, [isFinishing, sessionId]);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        cancel();
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [cancel]);

  function select(event: React.PointerEvent<HTMLDivElement>) {
    if (event.button !== 0 || isFinishing) return;
    event.preventDefault();
    setIsFinishing(true);
    void selectCoordinatePicker(sessionId);
  }

  return (
    <div
      aria-label="Screen coordinate picker"
      className="flex h-screen w-screen cursor-crosshair select-none items-center justify-center bg-black/30"
      onContextMenu={(event) => event.preventDefault()}
      onPointerDown={select}
      role="application"
    >
      <div className="pointer-events-none flex max-w-xs items-center gap-3 rounded-md border border-white/20 bg-black/75 px-4 py-3 text-white shadow-lg">
        <Crosshair className="size-5 shrink-0 text-baud-red" />
        <div>
          <div className="font-medium">Select a screen coordinate</div>
          <div className="mt-0.5 text-sm text-white/70">
            {isFinishing ? "Capturing selection..." : "Click to select. Press Esc to cancel."}
          </div>
        </div>
      </div>
    </div>
  );
}

import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { DesktopDialogConsoleView } from "@/views/desktop-dialog-view";

describe("desktop dialog console", () => {
  it("renders a stable waiting state before a request arrives", () => {
    const markup = renderToStaticMarkup(<DesktopDialogConsoleView />);

    expect(markup).toContain("Waiting for dialog requests");
    expect(markup).toContain("No dialog request is active.");
    expect(markup).toContain("data-desktop-dialog-shell");
    expect(markup).toContain("Dialog console menu");
    expect(markup).toContain("max-w-full");
    expect(markup).toContain("overflow-hidden");
  });
});

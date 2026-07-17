import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type {
  ApplicationSettingsPayload,
  TimeFormat,
} from "@/lib/runner-api";
import { SettingsView } from "@/views/settings-view";

function renderSettings(timeFormat: TimeFormat) {
  const payload: ApplicationSettingsPayload = {
    launch_at_login_registered: false,
    settings: {
      desktop: {
        automatic_update_checks: true,
        keep_running_on_close: true,
        launch_at_login: false,
        start_background_runner_on_launch: true,
        start_minimized_to_tray: false,
      },
      shared: { time_format: timeFormat },
    },
  };

  return renderToStaticMarkup(
    <SettingsView onSaved={() => undefined} payload={payload} />,
  );
}

describe("SettingsView", () => {
  it("keeps controls available in the minimum-width stacked layout", () => {
    const markup = renderSettings("24-hour");

    expect(markup).toContain("flex-wrap");
    expect(markup).toContain("max-sm:grid-cols-1");
    expect(markup).toContain("max-sm:justify-stretch");
    expect(markup).toContain("break-words");
    expect(markup).toContain("Save settings");
    expect(markup).not.toContain("max-w-4xl");
  });

  it.each(["12-hour", "24-hour"] as const)(
    "marks the %s clock option as selected",
    (timeFormat) => {
      const markup = renderSettings(timeFormat);
      const selectedOption = new RegExp(
        `aria-pressed="true"[^>]*>${timeFormat}</button>`,
      );

      expect(markup).toMatch(selectedOption);
    },
  );
});

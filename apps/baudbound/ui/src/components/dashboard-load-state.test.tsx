import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { DashboardLoadState } from "@/components/dashboard-load-state";

describe("DashboardLoadState", () => {
  it("renders the loading state before the first dashboard response", () => {
    const markup = renderToStaticMarkup(
      <DashboardLoadState error={null} onRetry={() => undefined} />,
    );

    expect(markup).toContain("Loading runner state...");
    expect(markup).not.toContain("Retry");
  });

  it("renders the backend error and retry command after loading fails", () => {
    const markup = renderToStaticMarkup(
      <DashboardLoadState error="Database could not be read" onRetry={() => undefined} />,
    );

    expect(markup).toContain("Runner state could not be loaded");
    expect(markup).toContain("Database could not be read");
    expect(markup).toContain("Retry");
  });
});

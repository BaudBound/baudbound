import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { StatusSummaryCard } from "@/components/status-summary-card";
import { Badge } from "@/components/ui/badge";

describe("bounded status labels", () => {
  it("allows a summary badge to move below its metric without leaving the card", () => {
    const markup = renderToStaticMarkup(
      <StatusSummaryCard label="Unprotected network" tone="destructive" value={12} />,
    );

    expect(markup).toContain("min-w-0 flex-wrap");
    expect(markup).toContain("max-w-full shrink-0");
    expect(markup).toContain("Unprotected network");
  });

  it("wraps long badge content within the available width", () => {
    const markup = renderToStaticMarkup(
      <Badge variant="muted">an_unexpectedly_long_permission_name_without_spaces</Badge>,
    );

    expect(markup).toContain("max-w-full");
    expect(markup).toContain("overflow-wrap:anywhere");
    expect(markup).not.toContain("whitespace-nowrap");
  });
});

import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { StatusSummaryCard } from "@/components/status-summary-card";
import { Badge } from "@/components/ui/badge";

describe("bounded status labels", () => {
  it("keeps a summary badge in a stable column without leaving the card", () => {
    const markup = renderToStaticMarkup(
      <StatusSummaryCard
        badgeLabel="Review"
        label="Unprotected network"
        tone="destructive"
        value={12}
      />,
    );

    expect(markup).toContain("grid-cols-[minmax(0,1fr)_auto]");
    expect(markup).toContain("overflow-hidden");
    expect(markup).toContain("text-ellipsis");
    expect(markup).toContain("whitespace-nowrap");
    expect(markup).toContain('title="Unprotected network"');
    expect(markup).toContain("Unprotected network");
    expect(markup).toContain("Review");
  });

  it("keeps badge content on one line and clips it within the available width", () => {
    const markup = renderToStaticMarkup(
      <Badge variant="muted">an_unexpectedly_long_permission_name_without_spaces</Badge>,
    );

    expect(markup).toContain("max-w-full");
    expect(markup).toContain("overflow-hidden");
    expect(markup).toContain("text-ellipsis");
    expect(markup).toContain("whitespace-nowrap");
  });
});

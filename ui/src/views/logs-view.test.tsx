import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { SystemLogSummary } from "@/lib/runner-api";
import { SystemLogUnreadBadges } from "@/views/logs-view";

describe("SystemLogUnreadBadges", () => {
  it("renders compact counts while retaining the complete severity descriptions", () => {
    const markup = renderToStaticMarkup(
      <SystemLogUnreadBadges
        summary={systemLogSummary({
          unread: 9,
          unread_errors: 2,
          unread_info: 5,
          unread_successes: 1,
          unread_warnings: 1,
        })}
      />,
    );

    expect(markup).toContain(">2</span>");
    expect(markup).toContain(">1</span>");
    expect(markup).toContain(">5</span>");
    expect(markup).toContain('aria-label="2 unread errors"');
    expect(markup).toContain("Unread system logs: 2 errors, 1 warning, 5 information, 1 success");
    expect(markup).not.toContain(">2 errors</span>");
  });

  it("caps large visible counts without hiding the exact accessible count", () => {
    const markup = renderToStaticMarkup(
      <SystemLogUnreadBadges summary={systemLogSummary({ unread: 140, unread_errors: 140 })} />,
    );

    expect(markup).toContain(">99+</span>");
    expect(markup).toContain('aria-label="140 unread errors"');
  });
});

function systemLogSummary(patch: Partial<SystemLogSummary>): SystemLogSummary {
  return {
    total: 0,
    unread: 0,
    unread_errors: 0,
    unread_info: 0,
    unread_successes: 0,
    unread_warnings: 0,
    ...patch,
  };
}

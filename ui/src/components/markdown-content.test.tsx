import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { MarkdownContent } from "@/components/markdown-content";

describe("MarkdownContent", () => {
  it("formats release notes without executing embedded HTML", () => {
    const markup = renderToStaticMarkup(
      <MarkdownContent source={'## Changes\n\n* Fixed imports\n\n<script>alert("no")</script>'} />,
    );

    expect(markup).toContain("Changes");
    expect(markup).toContain("Fixed imports");
    expect(markup).not.toContain("<script>");
  });
});

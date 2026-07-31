import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { NumericField } from "@/components/numeric-field";

describe("NumericField", () => {
  it("renders compact right-side controls after a left-aligned numeric editor", () => {
    const markup = renderToStaticMarkup(
      <NumericField
        ariaLabel="Port"
        contract={{ kind: "integer", maximum: "65535", minimum: "1", signed: false }}
        onChange={() => undefined}
        value={43891}
      />,
    );

    expect(markup).toContain('type="text"');
    expect(markup).not.toContain('type="number"');
    expect(markup).toContain('role="spinbutton"');
    expect(markup).toContain('aria-label="Decrease Port"');
    expect(markup).toContain('aria-label="Increase Port"');
    expect(markup).toContain("text-left");
    expect(markup).toContain("[&amp;_svg]:size-3");
    expect(markup.indexOf('role="spinbutton"')).toBeLessThan(
      markup.indexOf('aria-label="Decrease Port"'),
    );
    expect(markup.indexOf('aria-label="Decrease Port"')).toBeLessThan(
      markup.indexOf('aria-label="Increase Port"'),
    );
  });

  it("keeps native numeric inputs out of runner source", () => {
    const sources = import.meta.glob("../**/*.{ts,tsx}", {
      eager: true,
      import: "default",
      query: "?raw",
    }) as Record<string, string>;

    for (const [sourcePath, source] of Object.entries(sources)) {
      if (sourcePath.includes(".test.")) {
        continue;
      }
      expect(source, sourcePath).not.toContain('type="number"');
      expect(source, sourcePath).not.toContain("valueAsNumber");
      expect(source, sourcePath).not.toMatch(/type=\{[^\n}]*["']number["']/);
    }
  });
});

import { describe, expect, it } from "vitest";
import appSource from "../app.tsx?raw";
import badgeSource from "../components/ui/badge.tsx?raw";
import cardSource from "../components/ui/card.tsx?raw";
import browseScriptsViewSource from "../views/browse-scripts-view.tsx?raw";
import scriptRowSource from "../views/script-row.tsx?raw";
import scriptsViewSource from "../views/scripts-view.tsx?raw";

const sourceFiles = import.meta.glob("../**/*.{css,ts,tsx}", {
  eager: true,
  query: "?raw",
  import: "default",
}) as Record<string, string>;
const forbiddenOverflow = /\boverflow-(?:x-auto|x-scroll|scroll|auto)\b|overflow(?:-x)?\s*:\s*(?:auto|scroll)/;

describe("horizontal overflow contract", () => {
  it("forbids horizontal scrollbar utilities throughout the runner UI", () => {
    const offenders = Object.entries(sourceFiles).flatMap(([path, source]) =>
      forbiddenOverflow.test(source) ? [path] : [],
    );

    expect(offenders).toEqual([]);
  });

  it("keeps shared page and table containers width bounded", () => {
    expect(appSource).toContain("runner-content min-h-0 min-w-0 max-w-full");
    expect(cardSource).toContain("min-w-0 max-w-full rounded-lg");
  });

  it("uses content-sized script columns and keeps actions compact and responsive", () => {
    expect(scriptsViewSource).toContain("responsive-table scripts-table");
    expect(scriptsViewSource).not.toContain("<colgroup");
    expect(scriptsViewSource).toContain('className="hidden min-[1500px]:table-cell"');
    expect(scriptRowSource).toContain('className="hidden px-3 py-3 min-[1500px]:table-cell"');
    expect(scriptRowSource).toContain("flex w-56 justify-between");
    expect(scriptRowSource).toContain("max-[1280px]:flex-wrap");
    expect(browseScriptsViewSource).toContain('<col className="w-56" />');
    expect(browseScriptsViewSource).toContain("max-w-full flex-wrap justify-end gap-2");
  });

  it("preserves horizontal padding for every shared badge", () => {
    expect(badgeSource).toContain("rounded-full border px-2 text-center");
  });

  it("keeps long installed script identifiers on one truncated line", () => {
    expect(scriptRowSource).toContain("max-w-full truncate text-xs");
    expect(scriptRowSource).toContain("title={reference}");
  });
});

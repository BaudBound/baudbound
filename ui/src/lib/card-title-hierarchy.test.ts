import { describe, expect, it } from "vitest";

const viewSources = import.meta.glob("../views/**/*.tsx", {
  eager: true,
  query: "?raw",
  import: "default",
}) as Record<string, string>;

const cardHeaderPattern = /<CardHeader\b[^>]*>([\s\S]*?)<\/CardHeader>/g;
const cardTitlePattern = /<CardTitle\b[^>]*>([\s\S]*?)<\/CardTitle>/g;
const componentElementPattern = /<[A-Z][A-Za-z0-9]*(?:\s|\/|>)/;
const sizedComponentPattern = /<[A-Z][A-Za-z0-9]*\b[^>]*className="[^"]*\bsize-[^"]*"/;

describe("card title hierarchy", () => {
  it("keeps card titles free of decorative components", () => {
    const offenders = Object.entries(viewSources).flatMap(([path, source]) =>
      [...source.matchAll(cardTitlePattern)].flatMap((match) =>
        componentElementPattern.test(match[1]) ? [path] : [],
      ),
    );

    expect(offenders).toEqual([]);
  });

  it("does not place decorative icons before card titles", () => {
    const offenders = Object.entries(viewSources).flatMap(([path, source]) =>
      [...source.matchAll(cardHeaderPattern)].flatMap((match) => {
        const titleIndex = match[1].indexOf("<CardTitle");
        if (titleIndex < 0) return [];
        return sizedComponentPattern.test(match[1].slice(0, titleIndex)) ? [path] : [];
      }),
    );

    expect(offenders).toEqual([]);
  });
});

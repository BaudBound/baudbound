import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import { loadWikiPages } from "../src/content.mjs";

test("loads metadata and rewrites repository links to Wiki.js paths", async (context) => {
  const root = await fixtureRoot(context);
  await writePage(
    root,
    "home.md",
    pageSource("BaudBound", "Home page", "Read the [guide](guide/index.md#start)."),
  );
  await writePage(
    root,
    "guide/index.md",
    pageSource("Guide", "Getting started", "## Start\n\nVisit [BaudBound](../home.md)."),
  );

  const pages = await loadWikiPages(root);
  const home = pages.find((page) => page.path === "home");
  const guide = pages.find((page) => page.path === "guide");

  assert.match(home.content, /\[guide\]\(\/guide#start\)/);
  assert.match(guide.content, /\[BaudBound\]\(\/home\)/);
  assert.deepEqual(home.tags, ["baudbound-docs", "managed-by-git"]);
});

test("preserves Wiki.js tabset attributes", async (context) => {
  const root = await fixtureRoot(context);
  await writePage(
    root,
    "home.md",
    pageSource(
      "BaudBound",
      "Home page",
      "## Platforms {.tabset}\n\n### Windows\n\nWindows content.\n\n### Linux\n\nLinux content.",
    ),
  );

  const [home] = await loadWikiPages(root);

  assert.match(home.content, /^## Platforms \{\.tabset\}$/m);
  assert.match(home.content, /^### Windows$/m);
  assert.match(home.content, /^### Linux$/m);
});

test("rejects missing links and local images with source locations", async (context) => {
  const root = await fixtureRoot(context);
  await writePage(
    root,
    "home.md",
    pageSource(
      "BaudBound",
      "Home page",
      "[Missing](missing.md)\n\n![Local image](images/logo.png)",
    ),
  );

  await assert.rejects(
    loadWikiPages(root),
    /home\.md:1: link points to a missing Markdown page[\s\S]*local images are not supported/,
  );
});

test("rejects unknown frontmatter and duplicate published paths", async (context) => {
  const root = await fixtureRoot(context);
  await writePage(
    root,
    "home.md",
    `---\ntitle: Home\ndescription: Home\nunsupported: true\n---\nContent\n`,
  );
  await assert.rejects(loadWikiPages(root), /unknown frontmatter fields: unsupported/);

  await writePage(root, "home.md", pageSource("Home", "Home", "Content"));
  await writePage(root, "index.md", pageSource("Index", "Index", "Content"));
  await assert.rejects(loadWikiPages(root), /both publish to home/);
});

async function fixtureRoot(context) {
  const root = path.join(tmpdir(), `baudbound-wiki-content-${crypto.randomUUID()}`);
  await mkdir(root, { recursive: true });
  context.after(async () => {
    const { rm } = await import("node:fs/promises");
    await rm(root, { force: true, recursive: true });
  });
  return root;
}

async function writePage(root, relativePath, content) {
  const destination = path.join(root, relativePath);
  await mkdir(path.dirname(destination), { recursive: true });
  await writeFile(destination, content, "utf8");
}

function pageSource(title, description, content) {
  return `---\ntitle: ${title}\ndescription: ${description}\n---\n${content}\n`;
}

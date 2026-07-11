import assert from "node:assert/strict";
import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import { loadWikiNavigation, reconcileNavigation } from "../src/navigation.mjs";

test("loads a complete static navigation tree and maps page targets", async () => {
  const navigation = await loadNavigation({
    items: [
      { id: "start", kind: "header", label: "Start" },
      { id: "home", kind: "link", label: "Home", icon: "mdi-home", path: "home" },
      { id: "guide", kind: "link", label: "Guide", icon: "mdi-book", path: "guide" },
    ],
    locale: "en",
    mode: "STATIC",
  });

  assert.equal(navigation.mode, "STATIC");
  assert.equal(navigation.tree[0].items[1].targetType, "home");
  assert.equal(navigation.tree[0].items[1].target, "");
  assert.equal(navigation.tree[0].items[2].targetType, "page");
  assert.equal(navigation.tree[0].items[2].target, "/en/guide");
});

test("rejects duplicate ids, missing pages, and incomplete page coverage", async () => {
  await assert.rejects(
    loadNavigation({
      items: [
        { id: "same", kind: "link", label: "Home", icon: "mdi-home", path: "home" },
        { id: "same", kind: "link", label: "Guide", icon: "mdi-book", path: "guide" },
      ],
      locale: "en",
      mode: "STATIC",
    }),
    /duplicates same/,
  );
  await assert.rejects(
    loadNavigation({
      items: [{ id: "missing", kind: "link", label: "Missing", icon: "mdi-book", path: "missing" }],
      locale: "en",
      mode: "STATIC",
    }),
    /points to missing managed page/,
  );
  await assert.rejects(
    loadNavigation({
      items: [{ id: "home", kind: "link", label: "Home", icon: "mdi-home", path: "home" }],
      locale: "en",
      mode: "STATIC",
    }),
    /navigation is missing managed en pages: guide/,
  );
});

test("updates changed navigation once and preserves dry runs", async () => {
  const local = await loadNavigation({
    items: [
      { id: "home", kind: "link", label: "Home", icon: "mdi-home", path: "home" },
      { id: "guide", kind: "link", label: "Guide", icon: "mdi-book", path: "guide" },
    ],
    locale: "en",
    mode: "STATIC",
  });
  const client = fakeClient({ mode: "NONE", tree: [] });

  assert.deepEqual(await reconcileNavigation({ client, dryRun: true, navigation: local }), {
    changed: true,
    mode: "STATIC",
  });
  assert.equal(client.updates.length, 0);

  assert.equal((await reconcileNavigation({ client, navigation: local })).changed, true);
  assert.equal(client.updates.length, 1);
  assert.equal((await reconcileNavigation({ client, navigation: local })).changed, false);
  assert.equal(client.updates.length, 1);
});

async function loadNavigation(source) {
  const directory = await mkdtemp(path.join(tmpdir(), "baudbound-navigation-"));
  const sourcePath = path.join(directory, "navigation.json");
  await writeFile(sourcePath, JSON.stringify(source), "utf8");
  return loadWikiNavigation(sourcePath, [page("home"), page("guide")]);
}

function page(pagePath) {
  return { locale: "en", path: pagePath };
}

function fakeClient(initial) {
  return {
    current: structuredClone(initial),
    updates: [],
    async readNavigation() {
      return structuredClone(this.current);
    },
    async updateNavigation(value) {
      this.current = structuredClone(value);
      this.updates.push(structuredClone(value));
    },
  };
}

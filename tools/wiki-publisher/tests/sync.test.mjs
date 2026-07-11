import assert from "node:assert/strict";
import test from "node:test";

import { MANAGED_TAGS } from "../src/constants.mjs";
import { reconcileWiki } from "../src/sync.mjs";

test("creates, updates, deletes, and preserves unchanged managed pages", async () => {
  const unchanged = page("home", "Home");
  const updatedLocal = page("guide", "New guide");
  const updatedRemote = { ...page("guide", "Old guide"), id: 2 };
  const removed = { ...page("removed", "Removed"), id: 3 };
  const client = fakeClient([
    { ...unchanged, id: 1 },
    updatedRemote,
    removed,
  ]);

  const result = await reconcileWiki({
    client,
    localPages: [unchanged, updatedLocal, page("new-page", "New page")],
  });

  assert.deepEqual(result, {
    creates: ["en:new-page"],
    deletes: ["en:removed"],
    unchanged: 1,
    updates: ["en:guide"],
  });
  assert.deepEqual(client.operations, [
    "create:en:new-page",
    "update:2:en:guide",
    "delete:3:en:removed",
  ]);
});

test("requires explicit approval before adopting an unmanaged matching page", async () => {
  const unmanaged = { ...page("home", "Placeholder"), id: 1, tags: [] };
  const client = fakeClient([unmanaged]);

  await assert.rejects(
    reconcileWiki({ client, localPages: [page("home", "BaudBound")] }),
    /refusing to overwrite unmanaged Wiki\.js pages: en:home/,
  );

  const result = await reconcileWiki({
    allowAdopt: true,
    client,
    localPages: [page("home", "BaudBound")],
  });
  assert.deepEqual(result.updates, ["en:home"]);
  assert.deepEqual(client.operations, ["update:1:en:home"]);
});

test("never deletes unmanaged pages and blocks large managed deletions", async () => {
  const managed = Array.from({ length: 7 }, (_, index) => ({
    ...page(`page-${index}`, `Page ${index}`),
    id: index + 1,
  }));
  const unmanaged = { ...page("private-notes", "Private"), id: 20, tags: [] };
  const client = fakeClient([...managed, unmanaged]);

  await assert.rejects(
    reconcileWiki({ client, localPages: [managed[0]] }),
    /refusing to delete 6 of 7 managed pages/,
  );
  assert.deepEqual(client.operations, []);

  const approved = await reconcileWiki({
    allowMassDelete: true,
    client,
    localPages: [managed[0]],
  });
  assert.equal(approved.deletes.length, 6);
  assert.ok(client.operations.every((operation) => !operation.includes("private-notes")));
});

function page(path, title) {
  return {
    content: `# ${title}\n`,
    description: `${title} description`,
    editor: "markdown",
    isPrivate: false,
    isPublished: true,
    locale: "en",
    path,
    tags: [...MANAGED_TAGS],
    title,
  };
}

function fakeClient(remotePages) {
  return {
    operations: [],
    async listPages() {
      return remotePages.map(({ content, editor, ...summary }) => summary);
    },
    async readPage(id) {
      return structuredClone(remotePages.find((page) => page.id === id));
    },
    async createPage(value) {
      this.operations.push(`create:${value.locale}:${value.path}`);
    },
    async updatePage(id, value) {
      this.operations.push(`update:${id}:${value.locale}:${value.path}`);
    },
    async deletePage(value) {
      this.operations.push(`delete:${value.id}:${value.locale}:${value.path}`);
    },
  };
}

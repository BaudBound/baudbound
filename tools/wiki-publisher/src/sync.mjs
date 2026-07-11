import {
  MANAGED_TAGS,
  MASS_DELETE_COUNT,
  MASS_DELETE_RATIO,
} from "./constants.mjs";

export async function reconcileWiki({
  client,
  localPages,
  allowAdopt = false,
  allowMassDelete = false,
  dryRun = false,
}) {
  const remoteSummaries = await client.listPages();
  const localByKey = uniquePageMap(localPages, "local documentation");
  const ownedSummaries = remoteSummaries.filter((page) =>
    MANAGED_TAGS.every((tag) => page.tags?.includes(tag)),
  );
  const adoptableSummaries = remoteSummaries.filter(
    (page) =>
      !MANAGED_TAGS.every((tag) => page.tags?.includes(tag)) && localByKey.has(pageKey(page)),
  );
  if (adoptableSummaries.length > 0 && !allowAdopt) {
    throw new Error(
      `refusing to overwrite unmanaged Wiki.js pages: ${adoptableSummaries.map(pageLabel).join(", ")}; rerun once with explicit adoption approval`,
    );
  }
  const relevantSummaries = [...ownedSummaries, ...(allowAdopt ? adoptableSummaries : [])];
  const remotePages = await Promise.all(relevantSummaries.map((page) => client.readPage(page.id)));
  const remoteByKey = uniquePageMap(remotePages, "remote Wiki.js");

  const creates = localPages.filter((page) => !remoteByKey.has(pageKey(page)));
  const updates = localPages
    .filter((page) => {
      const remote = remoteByKey.get(pageKey(page));
      return remote && !pagesEqual(page, remote);
    })
    .map((page) => ({ local: page, remote: remoteByKey.get(pageKey(page)) }));
  const ownedKeys = new Set(ownedSummaries.map(pageKey));
  const deletes = remotePages.filter(
    (page) => ownedKeys.has(pageKey(page)) && !localByKey.has(pageKey(page)),
  );

  assertDeletionSafety(deletes, remotePages, allowMassDelete);

  if (!dryRun) {
    for (const page of creates) await client.createPage(page);
    for (const { local, remote } of updates) await client.updatePage(remote.id, local);
    for (const page of deletes) await client.deletePage(page);
  }

  return {
    creates: creates.map(pageLabel),
    deletes: deletes.map(pageLabel),
    unchanged: localPages.length - creates.length - updates.length,
    updates: updates.map(({ local }) => pageLabel(local)),
  };
}

function assertDeletionSafety(deletes, remotePages, allowMassDelete) {
  if (allowMassDelete || deletes.length === 0) return;
  const ratio = remotePages.length === 0 ? 0 : deletes.length / remotePages.length;
  if (
    deletes.length > MASS_DELETE_COUNT ||
    (remotePages.length >= 10 && ratio > MASS_DELETE_RATIO)
  ) {
    throw new Error(
      `refusing to delete ${deletes.length} of ${remotePages.length} managed pages; rerun with explicit mass deletion approval`,
    );
  }
}

function pagesEqual(local, remote) {
  return (
    local.content === remote.content &&
    local.description === remote.description &&
    local.editor === remote.editor &&
    local.isPrivate === remote.isPrivate &&
    local.isPublished === remote.isPublished &&
    local.path === remote.path &&
    local.title === remote.title &&
    sorted(local.tags).join("\0") === sorted(remote.tags).join("\0")
  );
}

function uniquePageMap(pages, source) {
  const map = new Map();
  for (const page of pages) {
    const key = pageKey(page);
    if (map.has(key)) throw new Error(`${source} contains duplicate page ${pageLabel(page)}`);
    map.set(key, page);
  }
  return map;
}

function pageKey(page) {
  return `${page.locale}\0${page.path}`;
}

function pageLabel(page) {
  return `${page.locale}:${page.path}`;
}

function sorted(values) {
  return [...(values ?? [])].sort();
}

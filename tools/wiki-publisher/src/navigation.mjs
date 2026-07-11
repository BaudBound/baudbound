import { readFile } from "node:fs/promises";

const NAVIGATION_MODES = new Set(["MIXED", "NONE", "STATIC", "TREE"]);
const ITEM_KINDS = new Set(["divider", "header", "link"]);

export async function loadWikiNavigation(sourcePath, pages) {
  const source = JSON.parse(await readFile(sourcePath, "utf8"));
  assertObject(source, "navigation");
  assertExactKeys(source, new Set(["items", "locale", "mode"]), "navigation");
  if (!NAVIGATION_MODES.has(source.mode)) {
    throw new Error(`navigation.mode must be one of ${[...NAVIGATION_MODES].join(", ")}`);
  }
  if (typeof source.locale !== "string" || !source.locale.trim()) {
    throw new Error("navigation.locale must be a non-empty string");
  }
  if (!Array.isArray(source.items) || source.items.length === 0) {
    throw new Error("navigation.items must be a non-empty array");
  }

  const pageKeys = new Set(pages.map((page) => `${page.locale}\0${page.path}`));
  const ids = new Set();
  const linkedPages = new Set();
  const items = source.items.map((item, index) => {
    const location = `navigation.items[${index}]`;
    assertObject(item, location);
    if (!ITEM_KINDS.has(item.kind)) {
      throw new Error(`${location}.kind must be divider, header, or link`);
    }
    if (typeof item.id !== "string" || !/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(item.id)) {
      throw new Error(`${location}.id must be a stable kebab-case identifier`);
    }
    if (ids.has(item.id)) throw new Error(`${location}.id duplicates ${item.id}`);
    ids.add(item.id);

    if (item.kind === "divider") {
      assertExactKeys(item, new Set(["id", "kind"]), location);
      return apiItem(item);
    }

    if (typeof item.label !== "string" || !item.label.trim()) {
      throw new Error(`${location}.label must be a non-empty string`);
    }
    if (item.kind === "header") {
      assertExactKeys(item, new Set(["id", "kind", "label"]), location);
      return apiItem(item);
    }

    assertExactKeys(item, new Set(["icon", "id", "kind", "label", "path"]), location);
    if (typeof item.icon !== "string" || !/^mdi-[a-z0-9-]+$/.test(item.icon)) {
      throw new Error(`${location}.icon must be a Material Design icon name beginning with mdi-`);
    }
    if (typeof item.path !== "string" || !item.path.trim()) {
      throw new Error(`${location}.path must be a non-empty Wiki.js page path`);
    }
    const pageKey = `${source.locale}\0${item.path}`;
    if (!pageKeys.has(pageKey)) {
      throw new Error(`${location}.path points to missing managed page ${source.locale}:${item.path}`);
    }
    if (linkedPages.has(pageKey)) {
      throw new Error(`${location}.path duplicates managed page ${source.locale}:${item.path}`);
    }
    linkedPages.add(pageKey);
    return apiItem(item, source.locale);
  });

  const missingPages = pages
    .filter((page) => page.locale === source.locale && !linkedPages.has(`${page.locale}\0${page.path}`))
    .map((page) => page.path);
  if (missingPages.length > 0) {
    throw new Error(`navigation is missing managed ${source.locale} pages: ${missingPages.join(", ")}`);
  }

  return { mode: source.mode, tree: [{ items, locale: source.locale }] };
}

export async function reconcileNavigation({
  client,
  dryRun = false,
  navigation,
  remoteNavigation,
}) {
  const remote = normalizeNavigation(remoteNavigation ?? (await client.readNavigation()));
  const local = normalizeNavigation(navigation);
  const changed = JSON.stringify(remote) !== JSON.stringify(local);
  if (changed && !dryRun) await client.updateNavigation(local);
  return { changed, mode: local.mode };
}

function apiItem(item, locale) {
  const isLink = item.kind === "link";
  const isHome = isLink && item.path === "home";
  return {
    icon: isLink ? item.icon : null,
    id: item.id,
    kind: item.kind,
    label: item.label ?? null,
    target: isLink && !isHome ? `/${locale}/${item.path}` : "",
    targetType: isLink ? (isHome ? "home" : "page") : null,
    visibilityGroups: [],
    visibilityMode: "all",
  };
}

function normalizeNavigation(value) {
  return {
    mode: value.mode,
    tree: [...value.tree]
      .map((tree) => ({
        items: tree.items.map((item) => ({
          icon: item.icon ?? null,
          id: item.id,
          kind: item.kind,
          label: item.label ?? null,
          target: item.target ?? "",
          targetType: item.targetType ?? null,
          visibilityGroups: [...(item.visibilityGroups ?? [])].sort((a, b) => a - b),
          visibilityMode: item.visibilityMode ?? "all",
        })),
        locale: tree.locale,
      }))
      .sort((a, b) => a.locale.localeCompare(b.locale)),
  };
}

function assertObject(value, location) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${location} must be an object`);
  }
}

function assertExactKeys(value, allowed, location) {
  const unknown = Object.keys(value).filter((key) => !allowed.has(key));
  if (unknown.length > 0) throw new Error(`${location} has unknown fields: ${unknown.join(", ")}`);
  const missing = [...allowed].filter((key) => !(key in value));
  if (missing.length > 0) throw new Error(`${location} is missing fields: ${missing.join(", ")}`);
}

import { readFile, stat } from "node:fs/promises";
import path from "node:path";

import fg from "fast-glob";
import matter from "gray-matter";
import { remark } from "remark";
import remarkParse from "remark-parse";
import remarkStringify from "remark-stringify";
import { visit } from "unist-util-visit";

import { DEFAULT_LOCALE, MANAGED_TAGS, sourcePathToWikiPath } from "./constants.mjs";

const markdownProcessor = remark().use(remarkParse).use(remarkStringify, {
  bullet: "-",
  fences: true,
  listItemIndent: "one",
});

const assetDirectory = "assets";
const supportedAssetExtensions = new Set([".jpeg", ".jpg", ".png", ".webp"]);
const maximumAssetBytes = 2 * 1024 * 1024;
const defaultAssetBaseUrl =
  "https://raw.githubusercontent.com/NATroutter/BaudBound/master/docs/wiki/assets";

export async function loadWikiPages(sourceRoot) {
  const relativePaths = await fg("**/*.md", {
    absolute: false,
    cwd: sourceRoot,
    onlyFiles: true,
  });
  if (relativePaths.length === 0) {
    throw new Error(`no Markdown pages found in ${sourceRoot}`);
  }

  const drafts = await Promise.all(
    relativePaths.sort().map((relativePath) => readPageDraft(sourceRoot, relativePath)),
  );
  const byWikiPath = new Map();
  const bySourcePath = new Map(drafts.map((draft) => [normalizeSourcePath(draft.relativePath), draft]));
  const assets = await loadAssets(sourceRoot);

  for (const draft of drafts) {
    const previous = byWikiPath.get(draft.path);
    if (previous) {
      throw new Error(
        `${draft.relativePath} and ${previous.relativePath} both publish to ${draft.path}`,
      );
    }
    byWikiPath.set(draft.path, draft);
  }

  return drafts.map((draft) => finalizePage(draft, bySourcePath, byWikiPath, assets));
}

async function loadAssets(sourceRoot) {
  const relativePaths = await fg(`${assetDirectory}/**/*`, {
    absolute: false,
    cwd: sourceRoot,
    onlyFiles: true,
  });
  const assets = new Map();
  const errors = [];

  await Promise.all(
    relativePaths.sort().map(async (relativePath) => {
      const normalizedPath = normalizeSourcePath(relativePath);
      const extension = path.posix.extname(normalizedPath).toLowerCase();
      if (!supportedAssetExtensions.has(extension)) {
        errors.push(`${normalizedPath}: unsupported wiki asset type ${extension || "(none)"}`);
        return;
      }
      const file = await stat(path.join(sourceRoot, relativePath));
      if (file.size > maximumAssetBytes) {
        errors.push(
          `${normalizedPath}: wiki asset is ${file.size} bytes; maximum is ${maximumAssetBytes}`,
        );
        return;
      }
      assets.set(normalizedPath, file.size);
    }),
  );

  if (errors.length > 0) throw new Error(errors.join("\n"));
  return assets;
}

async function readPageDraft(sourceRoot, relativePath) {
  const absolutePath = path.join(sourceRoot, relativePath);
  const source = await readFile(absolutePath, "utf8");
  const parsed = matter(source);
  const metadata = validateMetadata(relativePath, parsed.data);
  const wikiPath = sourcePathToWikiPath(relativePath);
  validateWikiPath(relativePath, wikiPath);

  if (!parsed.content.trim()) {
    throw new Error(`${relativePath}: page content must not be empty`);
  }

  return {
    ...metadata,
    content: parsed.content,
    path: wikiPath,
    relativePath,
  };
}

function finalizePage(draft, bySourcePath, byWikiPath, assets) {
  const tree = markdownProcessor.parse(draft.content);
  const errors = [];

  visit(tree, "link", (node) => {
    try {
      node.url = resolveLink(draft, node.url, bySourcePath, byWikiPath);
    } catch (error) {
      errors.push(formatNodeError(draft.relativePath, node, error.message));
    }
  });
  visit(tree, "image", (node) => {
    try {
      node.url = resolveImage(draft, node.url, assets);
    } catch (error) {
      errors.push(formatNodeError(draft.relativePath, node, error.message));
    }
  });

  if (errors.length > 0) {
    throw new Error(errors.join("\n"));
  }

  return {
    content: String(markdownProcessor.stringify(tree)).trimEnd() + "\n",
    description: draft.description,
    editor: "markdown",
    isPrivate: draft.isPrivate,
    isPublished: draft.isPublished,
    locale: draft.locale,
    path: draft.path,
    tags: [...new Set([...draft.tags, ...MANAGED_TAGS])].sort(),
    title: draft.title,
  };
}

function validateMetadata(relativePath, metadata) {
  const allowed = new Set(["description", "locale", "private", "published", "tags", "title"]);
  const unknown = Object.keys(metadata).filter((key) => !allowed.has(key));
  if (unknown.length > 0) {
    throw new Error(`${relativePath}: unknown frontmatter fields: ${unknown.join(", ")}`);
  }
  if (typeof metadata.title !== "string" || !metadata.title.trim()) {
    throw new Error(`${relativePath}: frontmatter title is required`);
  }
  if (typeof metadata.description !== "string" || !metadata.description.trim()) {
    throw new Error(`${relativePath}: frontmatter description is required`);
  }
  if (metadata.published !== undefined && typeof metadata.published !== "boolean") {
    throw new Error(`${relativePath}: published must be a boolean`);
  }
  if (metadata.private !== undefined && typeof metadata.private !== "boolean") {
    throw new Error(`${relativePath}: private must be a boolean`);
  }
  if (metadata.locale !== undefined && !/^[a-z]{2}(?:-[A-Z]{2})?$/.test(metadata.locale)) {
    throw new Error(`${relativePath}: locale must use a language or language-region code`);
  }
  if (metadata.tags !== undefined && !isStringArray(metadata.tags)) {
    throw new Error(`${relativePath}: tags must be an array of non-empty strings`);
  }

  return {
    description: metadata.description.trim(),
    isPrivate: metadata.private ?? false,
    isPublished: metadata.published ?? true,
    locale: metadata.locale ?? DEFAULT_LOCALE,
    tags: (metadata.tags ?? []).map((tag) => tag.trim().toLowerCase()),
    title: metadata.title.trim(),
  };
}

function validateWikiPath(relativePath, wikiPath) {
  if (!/^[a-z0-9]+(?:-[a-z0-9]+)*(?:\/[a-z0-9]+(?:-[a-z0-9]+)*)*$/.test(wikiPath)) {
    throw new Error(`${relativePath}: generated Wiki.js path is invalid: ${wikiPath}`);
  }
  if (/^[a-z]$/.test(wikiPath)) {
    throw new Error(`${relativePath}: single-character Wiki.js paths are reserved`);
  }
}

function resolveLink(draft, url, bySourcePath, byWikiPath) {
  if (isExternalUrl(url)) {
    validateExternalUrl(url, { allowContactSchemes: true });
    return url;
  }
  if (url.startsWith("#")) {
    validateAnchor(draft, url);
    return url;
  }
  const { pathname, suffix } = splitLink(url);

  if (pathname.startsWith("/")) {
    const wikiPath = pathname.replace(/^\/+|\/+$/g, "") || "home";
    if (!byWikiPath.has(wikiPath)) {
      throw new Error(`link points to an unmanaged or missing wiki page: ${url}`);
    }
    validateAnchor(byWikiPath.get(wikiPath), suffix);
    return `/${wikiPath}${suffix}`;
  }

  const sourceDirectory = path.posix.dirname(normalizeSourcePath(draft.relativePath));
  let targetSourcePath = path.posix.normalize(path.posix.join(sourceDirectory, pathname));
  if (!/\.md$/i.test(targetSourcePath)) {
    const direct = `${targetSourcePath}.md`;
    const indexed = path.posix.join(targetSourcePath, "index.md");
    targetSourcePath = bySourcePath.has(direct) ? direct : indexed;
  }
  const target = bySourcePath.get(targetSourcePath);
  if (!target) {
    throw new Error(`link points to a missing Markdown page: ${url}`);
  }
  validateAnchor(target, suffix);
  return `/${target.path}${suffix}`;
}

function validateAnchor(target, suffix) {
  const fragment = suffix.match(/#([^?]+)/)?.[1];
  if (!fragment) return;
  const requested = decodeURIComponent(fragment).toLowerCase();
  const tree = markdownProcessor.parse(target.content);
  const anchors = new Set();
  visit(tree, "heading", (node) => {
    anchors.add(headingSlug(nodeText(node)));
  });
  if (!anchors.has(requested)) {
    throw new Error(`link points to a missing heading #${fragment} in ${target.relativePath}`);
  }
}

function nodeText(node) {
  if (typeof node.value === "string") return node.value;
  return (node.children ?? []).map(nodeText).join("");
}

function headingSlug(value) {
  return value
    .replace(/\s+\{\.[^}]+\}\s*$/, "")
    .trim()
    .toLowerCase()
    .replace(/[^\p{Letter}\p{Number}\s_-]/gu, "")
    .replace(/\s+/g, "-");
}

function resolveImage(draft, url, assets) {
  if (isExternalUrl(url)) {
    validateExternalUrl(url, { allowContactSchemes: false });
    return url;
  }

  const { pathname, suffix } = splitLink(url);
  const sourceDirectory = path.posix.dirname(normalizeSourcePath(draft.relativePath));
  const targetPath = path.posix.normalize(path.posix.join(sourceDirectory, pathname));
  if (!targetPath.startsWith(`${assetDirectory}/`)) {
    throw new Error(`local images must be stored beneath ${assetDirectory}/: ${url}`);
  }
  if (!assets.has(targetPath)) {
    throw new Error(`image points to a missing or invalid wiki asset: ${url}`);
  }

  const baseUrl = (process.env.WIKI_ASSET_BASE_URL ?? defaultAssetBaseUrl).replace(/\/$/, "");
  const relativeAssetPath = targetPath.slice(assetDirectory.length + 1);
  const encodedPath = relativeAssetPath.split("/").map(encodeURIComponent).join("/");
  return `${baseUrl}/${encodedPath}${suffix}`;
}

function validateExternalUrl(url, { allowContactSchemes }) {
  if (/^https:/i.test(url)) return;
  if (allowContactSchemes && /^(?:mailto|tel):/i.test(url)) return;
  throw new Error(`external URLs must use HTTPS: ${url}`);
}

function splitLink(url) {
  const index = url.search(/[?#]/);
  return index === -1
    ? { pathname: url, suffix: "" }
    : { pathname: url.slice(0, index), suffix: url.slice(index) };
}

function isExternalUrl(url) {
  return /^(?:https?:|mailto:|tel:)/i.test(url);
}

function isStringArray(value) {
  return (
    Array.isArray(value) &&
    value.every((entry) => typeof entry === "string" && entry.trim().length > 0)
  );
}

function normalizeSourcePath(value) {
  return value.replaceAll("\\", "/");
}

function formatNodeError(relativePath, node, message) {
  const line = node.position?.start.line;
  return `${relativePath}${line ? `:${line}` : ""}: ${message}`;
}

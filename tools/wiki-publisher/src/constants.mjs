export const DEFAULT_LOCALE = "en";
export const MANAGED_TAGS = Object.freeze(["baudbound-docs", "managed-by-git"]);
export const MASS_DELETE_COUNT = 5;
export const MASS_DELETE_RATIO = 0.25;

export function sourcePathToWikiPath(relativePath) {
  const normalized = relativePath.replaceAll("\\", "/");
  const withoutExtension = normalized.replace(/\.md$/i, "");
  const withoutIndex = withoutExtension.replace(/(?:^|\/)index$/i, "");
  const path = withoutIndex.replace(/^\/+|\/+$/g, "");
  return path || "home";
}

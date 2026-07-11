import path from "node:path";
import { fileURLToPath } from "node:url";

import { loadWikiPages } from "./content.mjs";
import { WikiJsClient } from "./graphql-client.mjs";
import { loadWikiNavigation, reconcileNavigation } from "./navigation.mjs";
import { reconcileWiki } from "./sync.mjs";

const [command = "validate"] = process.argv.slice(2);
const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
const sourceRoot = path.resolve(repositoryRoot, process.env.WIKI_SOURCE_ROOT ?? "docs/wiki");
const navigationPath = path.resolve(
  repositoryRoot,
  process.env.WIKI_NAVIGATION_SOURCE ?? "docs/wiki/navigation.json",
);

try {
  const pages = await loadWikiPages(sourceRoot);
  const navigation = await loadWikiNavigation(navigationPath, pages);
  if (command === "validate") {
    console.log(
      `Validated ${pages.length} Wiki.js pages and ${navigation.tree[0].items.length} navigation items.`,
    );
  } else if (command === "publish") {
    const client = new WikiJsClient({
      baseUrl: requiredEnvironment("WIKI_URL"),
      token: requiredEnvironment("WIKI_API_TOKEN"),
    });
    const dryRun = process.env.WIKI_DRY_RUN === "true";
    const remoteNavigation = await client.readNavigation();
    const pageResult = await reconcileWiki({
      allowAdopt: process.env.WIKI_ALLOW_ADOPT === "true",
      allowMassDelete: process.env.WIKI_ALLOW_MASS_DELETE === "true",
      client,
      dryRun,
      localPages: pages,
    });
    const navigationResult = await reconcileNavigation({
      client,
      dryRun,
      navigation,
      remoteNavigation,
    });
    printResult(pageResult, navigationResult, dryRun);
  } else {
    throw new Error(`unknown command ${JSON.stringify(command)}; expected validate or publish`);
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}

function requiredEnvironment(name) {
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`${name} is required`);
  return value;
}

function printResult(result, navigation, dryRun) {
  const prefix = dryRun ? "Wiki.js dry run" : "Wiki.js publish";
  console.log(
    `${prefix}: ${result.creates.length} created, ${result.updates.length} updated, ${result.deletes.length} deleted, ${result.unchanged} unchanged.`,
  );
  for (const [operation, pages] of Object.entries(result)) {
    if (operation === "unchanged") continue;
    for (const page of pages) console.log(`${operation}: ${page}`);
  }
  console.log(
    `navigation: ${navigation.changed ? (dryRun ? "would update" : "updated") : "unchanged"} (${navigation.mode})`,
  );
}

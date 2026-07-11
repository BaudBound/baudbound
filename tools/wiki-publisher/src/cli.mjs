import path from "node:path";
import { fileURLToPath } from "node:url";

import { loadWikiPages } from "./content.mjs";
import { WikiJsClient } from "./graphql-client.mjs";
import { reconcileWiki } from "./sync.mjs";

const [command = "validate"] = process.argv.slice(2);
const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");
const sourceRoot = path.resolve(repositoryRoot, process.env.WIKI_SOURCE_ROOT ?? "docs/wiki");

try {
  const pages = await loadWikiPages(sourceRoot);
  if (command === "validate") {
    console.log(`Validated ${pages.length} Wiki.js pages from ${sourceRoot}.`);
  } else if (command === "publish") {
    const client = new WikiJsClient({
      baseUrl: requiredEnvironment("WIKI_URL"),
      token: requiredEnvironment("WIKI_API_TOKEN"),
    });
    const result = await reconcileWiki({
      allowAdopt: process.env.WIKI_ALLOW_ADOPT === "true",
      allowMassDelete: process.env.WIKI_ALLOW_MASS_DELETE === "true",
      client,
      dryRun: process.env.WIKI_DRY_RUN === "true",
      localPages: pages,
    });
    printResult(result, process.env.WIKI_DRY_RUN === "true");
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

function printResult(result, dryRun) {
  const prefix = dryRun ? "Wiki.js dry run" : "Wiki.js publish";
  console.log(
    `${prefix}: ${result.creates.length} created, ${result.updates.length} updated, ${result.deletes.length} deleted, ${result.unchanged} unchanged.`,
  );
  for (const [operation, pages] of Object.entries(result)) {
    if (operation === "unchanged") continue;
    for (const page of pages) console.log(`${operation}: ${page}`);
  }
}

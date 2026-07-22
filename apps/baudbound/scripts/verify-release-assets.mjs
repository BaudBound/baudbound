import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { validateReleaseAssets } from "./release-assets.mjs";

const [directory, tag, repository = "BaudBound/baudbound", releaseAssetsPath] = process.argv.slice(2);

try {
  const releaseAssets = releaseAssetsPath
    ? JSON.parse(readFileSync(resolve(releaseAssetsPath), "utf8")).assets
    : [];
  const result = validateReleaseAssets({
    directory: resolve(directory ?? ""),
    releaseAssets,
    repository,
    tag,
  });
  console.log(
    `Release ${tag} passed artifact validation for ${result.platforms.join(", ")} ` +
      `with ${result.assets.length} assets.`,
  );
} catch (error) {
  console.error(error.message);
  process.exit(1);
}

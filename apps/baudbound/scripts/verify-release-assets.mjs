import { resolve } from "node:path";
import { validateReleaseAssets } from "./release-assets.mjs";

const [directory, tag, repository = "NATroutter/BaudBound"] = process.argv.slice(2);

try {
  const result = validateReleaseAssets({ directory: resolve(directory ?? ""), repository, tag });
  console.log(
    `Release ${tag} passed artifact validation for ${result.platforms.join(", ")} ` +
      `with ${result.assets.length} assets.`,
  );
} catch (error) {
  console.error(error.message);
  process.exit(1);
}

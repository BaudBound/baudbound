import { resolve } from "node:path";
import { verifyLinuxPackages } from "./verify-linux-packages.mjs";

const [directory, tag] = process.argv.slice(2);

try {
  const result = verifyLinuxPackages({ directory: resolve(directory ?? ""), tag });
  const summary = result.packages
    .map((entry) => `${entry.deb} and ${entry.rpm} (${entry.architecture})`)
    .join(", ");
  console.log(`Linux package contracts passed for ${summary} at version ${result.version}.`);
} catch (error) {
  console.error(error.message);
  process.exit(1);
}

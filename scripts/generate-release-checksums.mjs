import { readdirSync, statSync } from "node:fs";
import { resolve } from "node:path";
import { writeChecksumManifest } from "./release-checksums.mjs";

const directory = resolve(process.argv[2] ?? "");

try {
  const assets = new Map();
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (!entry.isFile()) continue;
    const path = resolve(directory, entry.name);
    if (statSync(path).size > 0) assets.set(entry.name, path);
  }
  const name = writeChecksumManifest(directory, assets);
  console.log(`Generated ${name} for release installers.`);
} catch (error) {
  console.error(error.message);
  process.exit(1);
}

import { readdirSync, statSync } from "node:fs";
import { resolve } from "node:path";
import { writeChecksumManifest } from "./release-checksums.mjs";

const directory = resolve(process.argv[2] ?? "");
const tag = process.argv[3] ?? "";
const version = /^v(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$/.exec(tag)?.[1];

if (!version) {
  console.error(`release tag is required as the second argument, received ${tag || "nothing"}`);
  process.exit(1);
}

try {
  const assets = new Map();
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (!entry.isFile()) continue;
    const path = resolve(directory, entry.name);
    if (statSync(path).size > 0) assets.set(entry.name, path);
  }
  const name = writeChecksumManifest(directory, assets, version);
  console.log(`Generated ${name} for release installers.`);
} catch (error) {
  console.error(error.message);
  process.exit(1);
}

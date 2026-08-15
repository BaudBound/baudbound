// Composes latest.json once, after every package job has uploaded its bundles
// and signatures.
//
// Each package job used to write this file itself, which meant three jobs
// merging into one asset and racing to do it. Composing it here removes the
// race rather than narrowing it, and leaves generation and verification as
// independent implementations: scripts/release-assets.mjs checks the result
// without sharing any code with it.

import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { RELEASE_ARCHITECTURES, WINDOWS_UPDATER_KEY } from "./release-architectures.mjs";

export class UpdaterManifestError extends Error {
  constructor(message) {
    super(message);
    this.name = "UpdaterManifestError";
  }
}

export function buildUpdaterManifest({ directory, notes, publishedAt, repository, tag }) {
  const version = releaseVersion(tag);
  const names = readdirSync(directory, { withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => entry.name);

  const platforms = {};
  for (const { key, matches } of updaterPayloads()) {
    const payload = names.find((name) => !name.endsWith(".sig") && matches(name));
    assert(payload, `no updater payload for ${key}`);
    assert(names.includes(`${payload}.sig`), `no signature for ${payload}`);

    platforms[key] = {
      signature: readFileSync(resolve(directory, `${payload}.sig`), "utf8").trim(),
      url: `https://github.com/${repository}/releases/download/${tag}/${encodeURIComponent(payload)}`,
    };
  }

  return { version, notes: notes ?? "", pub_date: publishedAt, platforms };
}

function updaterPayloads() {
  return [
    {
      key: WINDOWS_UPDATER_KEY,
      matches: (name) => name.endsWith("-setup.exe") || name.endsWith(".nsis.zip"),
    },
    // An architecture with no AppImage declares no updater key either, so it
    // drops out of the manifest with the rest of its updater contract.
    ...RELEASE_ARCHITECTURES.filter((architecture) => architecture.updaterKey).map(
      (architecture) => ({
        key: architecture.updaterKey,
        matches: (name) =>
          name.endsWith(`_${architecture.appImage}.AppImage`) ||
          name.endsWith(`_${architecture.appImage}.AppImage.tar.gz`),
      }),
    ),
  ];
}

function releaseVersion(tag) {
  const match = /^v(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$/.exec(tag ?? "");
  assert(match, `invalid release tag ${tag}`);
  return match[1];
}

function assert(condition, message) {
  if (!condition) throw new UpdaterManifestError(message);
}

if (import.meta.filename === process.argv[1]) {
  const [directory, tag, repository = "BaudBound/baudbound", notes = ""] = process.argv.slice(2);
  try {
    const target = resolve(directory ?? "");
    const manifest = buildUpdaterManifest({
      directory: target,
      notes,
      publishedAt: new Date().toISOString(),
      repository,
      tag,
    });
    writeFileSync(resolve(target, "latest.json"), `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
    console.log(`Composed latest.json for ${Object.keys(manifest.platforms).join(", ")}.`);
  } catch (error) {
    console.error(error.message);
    process.exit(1);
  }
}

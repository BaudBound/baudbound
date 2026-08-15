import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { basename, resolve } from "node:path";
import { expectedArtifacts } from "./release-architectures.mjs";

export const CHECKSUM_FILENAME = "SHA256SUMS";

export function installerAssetNames(assets, version) {
  const names = [...assets.keys()];

  return expectedArtifacts(version).map((artifact) => {
    const matching = names.filter((name) => artifact.matches(name));
    assert(
      matching.length === 1,
      `release must contain exactly one ${artifact.label}, found ${matching.length}`,
    );
    assert(
      matching[0].includes(version),
      `${artifact.label} filename does not contain version ${version}`,
    );
    return matching[0];
  });
}

export function checksumFile(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

export function checksumManifest(assets, version) {
  return installerAssetNames(assets, version)
    .sort()
    .map((name) => `${checksumFile(assets.get(name))}  ${name}`)
    .join("\n") + "\n";
}

export function validateChecksumManifest(assets, version) {
  assert(assets.has(CHECKSUM_FILENAME), `release is missing ${CHECKSUM_FILENAME}`);
  const expectedNames = new Set(installerAssetNames(assets, version));
  const seen = new Set();
  const contents = readFileSync(assets.get(CHECKSUM_FILENAME), "utf8");

  for (const [index, line] of contents.split(/\r?\n/).entries()) {
    if (!line) continue;
    const match = /^([0-9a-f]{64})  ([A-Za-z0-9._+()-]+)$/.exec(line);
    assert(match, `${CHECKSUM_FILENAME} line ${index + 1} is invalid`);
    const [, expectedHash, name] = match;
    assert(expectedNames.has(name), `${CHECKSUM_FILENAME} contains unexpected asset ${name}`);
    assert(!seen.has(name), `${CHECKSUM_FILENAME} repeats asset ${name}`);
    assert(checksumFile(assets.get(name)) === expectedHash, `${name} checksum does not match ${CHECKSUM_FILENAME}`);
    seen.add(name);
  }

  for (const name of expectedNames) {
    assert(seen.has(name), `${CHECKSUM_FILENAME} is missing ${name}`);
  }
}

export function writeChecksumManifest(directory, assets, version) {
  const path = resolve(directory, CHECKSUM_FILENAME);
  writeFileSync(path, checksumManifest(assets, version), "utf8");
  return basename(path);
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

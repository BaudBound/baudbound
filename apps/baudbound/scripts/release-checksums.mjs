import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { basename, resolve } from "node:path";

export const CHECKSUM_FILENAME = "SHA256SUMS";

export function installerAssetNames(assets) {
  const names = [...assets.keys()];
  const definitions = [
    ["Windows NSIS installer", (name) => name.endsWith("-setup.exe")],
    ["Linux AppImage", (name) => name.endsWith(".AppImage")],
    ["Linux Debian package", (name) => name.endsWith(".deb")],
    ["Linux RPM package", (name) => name.endsWith(".rpm")],
  ];

  return definitions.map(([label, matches]) => {
    const matching = names.filter(matches);
    assert(matching.length === 1, `release must contain exactly one ${label}`);
    return matching[0];
  });
}

export function checksumFile(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

export function checksumManifest(assets) {
  return installerAssetNames(assets)
    .sort()
    .map((name) => `${checksumFile(assets.get(name))}  ${name}`)
    .join("\n") + "\n";
}

export function validateChecksumManifest(assets) {
  assert(assets.has(CHECKSUM_FILENAME), `release is missing ${CHECKSUM_FILENAME}`);
  const expectedNames = new Set(installerAssetNames(assets));
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

export function writeChecksumManifest(directory, assets) {
  const path = resolve(directory, CHECKSUM_FILENAME);
  writeFileSync(path, checksumManifest(assets), "utf8");
  return basename(path);
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

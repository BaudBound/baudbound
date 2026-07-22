import { readFileSync, readdirSync, statSync } from "node:fs";
import { basename, resolve } from "node:path";
import { validateChecksumManifest } from "./release-checksums.mjs";

const SUPPORTED_PLATFORMS = ["windows", "linux"];

export function validateReleaseAssets({ directory, releaseAssets = [], repository, tag }) {
  const version = validateInputs({ directory, repository, tag });
  const assets = readAssets(directory);
  const releaseAssetsByApiUrl = indexReleaseAssets(releaseAssets);
  requireInstallers(assets, version);
  validateChecksumManifest(assets);

  const manifest = readManifest(directory, assets);
  assert(manifest.version === version, `latest.json version must be ${version}`);
  assertValidDate(manifest.pub_date);
  assert(isRecord(manifest.platforms), "latest.json platforms must be an object");

  const platformNames = Object.keys(manifest.platforms);
  for (const supported of SUPPORTED_PLATFORMS) {
    assert(
      platformNames.some((name) => platformFamily(name) === supported),
      `latest.json is missing a ${supported} updater entry`,
    );
  }

  for (const platformName of platformNames) {
    const family = platformFamily(platformName);
    assert(family, `latest.json contains unsupported platform ${platformName}`);
    validatePlatform({
      assets,
      entry: manifest.platforms[platformName],
      family,
      platformName,
      releaseAssetsByApiUrl,
      repository,
      tag,
    });
  }

  return {
    assets: [...assets.keys()].sort(),
    platforms: platformNames.sort(),
    version,
  };
}

function validateInputs({ directory, repository, tag }) {
  assert(directory, "release asset directory is required");
  assert(/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(tag), "release tag is invalid");
  assert(/^[\w.-]+\/[\w.-]+$/.test(repository), "GitHub repository must use owner/name syntax");
  assert(statSync(directory).isDirectory(), `release asset directory does not exist: ${directory}`);
  return tag.slice(1);
}

function readAssets(directory) {
  const assets = new Map();
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (!entry.isFile()) continue;
    const path = resolve(directory, entry.name);
    assert(statSync(path).size > 0, `release asset is empty: ${entry.name}`);
    assets.set(entry.name, path);
  }
  return assets;
}

function requireInstallers(assets, version) {
  const names = [...assets.keys()];
  const expected = [
    ["Windows NSIS installer", (name) => name.endsWith("-setup.exe")],
    ["Linux AppImage", (name) => name.endsWith(".AppImage")],
    ["Linux Debian package", (name) => name.endsWith(".deb")],
    ["Linux RPM package", (name) => name.endsWith(".rpm")],
  ];

  for (const [label, matches] of expected) {
    const installers = names.filter(matches);
    assert(installers.length === 1, `release must contain exactly one ${label}`);
    assert(installers[0].includes(version), `${label} filename does not contain version ${version}`);
  }
}

function readManifest(directory, assets) {
  assert(assets.has("latest.json"), "release is missing latest.json");
  try {
    const value = JSON.parse(readFileSync(resolve(directory, "latest.json"), "utf8"));
    assert(isRecord(value), "latest.json root must be an object");
    return value;
  } catch (error) {
    if (error instanceof ReleaseAssetError) throw error;
    throw new ReleaseAssetError(`latest.json is not valid JSON: ${error.message}`);
  }
}

function validatePlatform({
  assets,
  entry,
  family,
  platformName,
  releaseAssetsByApiUrl,
  repository,
  tag,
}) {
  assert(isRecord(entry), `updater platform ${platformName} must be an object`);
  assert(typeof entry.signature === "string" && entry.signature.trim(), `${platformName} signature is missing`);

  let url;
  try {
    url = new URL(entry.url);
  } catch {
    throw new ReleaseAssetError(`${platformName} URL is invalid`);
  }

  assert(url.protocol === "https:", `${platformName} URL must use HTTPS`);
  const assetName = resolveUpdaterAssetName({
    platformName,
    releaseAssetsByApiUrl,
    repository,
    tag,
    url,
  });
  assert(assets.has(assetName), `${platformName} URL points to missing asset ${assetName}`);
  assert(
    isUpdaterPayload(family, assetName),
    `${platformName} URL points to the wrong updater payload type`,
  );

  const signatureName = `${assetName}.sig`;
  assert(assets.has(signatureName), `${platformName} updater asset is missing ${signatureName}`);
  const signature = readFileSync(assets.get(signatureName), "utf8").trim();
  assert(signature === entry.signature.trim(), `${platformName} signature does not match ${signatureName}`);
}

function resolveUpdaterAssetName({ platformName, releaseAssetsByApiUrl, repository, tag, url }) {
  if (url.hostname === "github.com") {
    const expectedPrefix = `/${repository}/releases/download/${tag}/`;
    assert(url.pathname.startsWith(expectedPrefix), `${platformName} URL must target ${repository} release ${tag}`);
    return decodeURIComponent(basename(url.pathname));
  }

  if (url.hostname === "api.github.com") {
    const expectedPath = new RegExp(`^/repos/${escapeRegExp(repository)}/releases/assets/\\d+$`);
    assert(expectedPath.test(url.pathname), `${platformName} API URL must target ${repository}`);
    const assetName = releaseAssetsByApiUrl.get(url.href);
    assert(assetName, `${platformName} API URL does not match an uploaded release asset`);
    return assetName;
  }

  throw new ReleaseAssetError(`${platformName} URL must use github.com or api.github.com`);
}

function indexReleaseAssets(releaseAssets) {
  assert(Array.isArray(releaseAssets), "release asset metadata must be an array");
  const byApiUrl = new Map();
  for (const asset of releaseAssets) {
    assert(isRecord(asset), "release asset metadata entry must be an object");
    assert(typeof asset.name === "string" && asset.name.trim(), "release asset metadata name is missing");
    assert(typeof asset.apiUrl === "string" && asset.apiUrl.trim(), "release asset metadata API URL is missing");
    assert(!byApiUrl.has(asset.apiUrl), `release asset metadata repeats API URL ${asset.apiUrl}`);
    byApiUrl.set(asset.apiUrl, asset.name);
  }
  return byApiUrl;
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function isUpdaterPayload(family, assetName) {
  if (family === "windows") {
    return assetName.endsWith(".nsis.zip") || assetName.endsWith("-setup.exe");
  }
  return assetName.endsWith(".AppImage.tar.gz") || assetName.endsWith(".AppImage");
}

function platformFamily(name) {
  return SUPPORTED_PLATFORMS.find((platform) => name.toLowerCase().startsWith(platform));
}

function assertValidDate(value) {
  assert(typeof value === "string" && value.trim(), "latest.json pub_date is missing");
  assert(!Number.isNaN(Date.parse(value)), "latest.json pub_date is invalid");
}

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function assert(condition, message) {
  if (!condition) throw new ReleaseAssetError(message);
}

export class ReleaseAssetError extends Error {
  constructor(message) {
    super(message);
    this.name = "ReleaseAssetError";
  }
}

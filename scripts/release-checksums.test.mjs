import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import assert from "node:assert/strict";
import { installerAssetNames } from "./release-checksums.mjs";

const VERSION = "2.0.0";

const ALL_ASSETS = [
  "BaudBound_2.0.0_x64-setup.exe",
  "Baudbound_2.0.0_amd64.AppImage",
  "Baudbound_2.0.0_amd64.deb",
  "Baudbound-2.0.0-1.x86_64.rpm",
  "Baudbound_2.0.0_aarch64.AppImage",
  "Baudbound_2.0.0_arm64.deb",
  "Baudbound-2.0.0-1.aarch64.rpm",
];

test("returns every declared artifact for a complete release", (context) => {
  const assets = createAssets(context, ALL_ASSETS);
  assert.deepEqual(installerAssetNames(assets, VERSION).sort(), [...ALL_ASSETS].sort());
});

test("rejects a release missing the ARM64 Debian package", (context) => {
  const assets = createAssets(
    context,
    ALL_ASSETS.filter((name) => name !== "Baudbound_2.0.0_arm64.deb"),
  );

  assert.throws(
    () => installerAssetNames(assets, VERSION),
    /exactly one Linux Debian package \(aarch64\)/,
  );
});

test("rejects a release carrying two artifacts for one architecture", (context) => {
  const assets = createAssets(context, [...ALL_ASSETS, "Baudbound_2.0.0-rc1_amd64.AppImage"]);

  assert.throws(
    () => installerAssetNames(assets, VERSION),
    /exactly one Linux AppImage \(x86_64\), found 2/,
  );
});

function createAssets(context, names) {
  const directory = mkdtempSync(join(tmpdir(), "baudbound-checksum-test-"));
  context.after(() => rmSync(directory, { force: true, recursive: true }));

  const assets = new Map();
  for (const name of names) {
    const path = join(directory, name);
    writeFileSync(path, name, "utf8");
    assets.set(name, path);
  }
  return assets;
}

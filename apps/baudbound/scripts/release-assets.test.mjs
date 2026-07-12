import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import assert from "node:assert/strict";
import { validateReleaseAssets } from "./release-assets.mjs";

const TAG = "v2.0.0";
const REPOSITORY = "NATroutter/BaudBound";

test("accepts matching Windows and Linux updater artifacts", (context) => {
  const directory = createRelease(context);
  const result = validateReleaseAssets({ directory, repository: REPOSITORY, tag: TAG });

  assert.deepEqual(result.platforms, ["linux-x86_64", "windows-x86_64"]);
  assert.equal(result.version, "2.0.0");
});

test("rejects a manifest signature that differs from the uploaded signature", (context) => {
  const directory = createRelease(context, (manifest) => {
    manifest.platforms["windows-x86_64"].signature = "tampered";
  });

  assert.throws(
    () => validateReleaseAssets({ directory, repository: REPOSITORY, tag: TAG }),
    /signature does not match/,
  );
});

test("rejects updater URLs for another release tag", (context) => {
  const directory = createRelease(context, (manifest) => {
    manifest.platforms["linux-x86_64"].url = manifest.platforms["linux-x86_64"].url.replace(TAG, "v9.9.9");
  });

  assert.throws(
    () => validateReleaseAssets({ directory, repository: REPOSITORY, tag: TAG }),
    /must target NATroutter\/BaudBound release v2\.0\.0/,
  );
});

test("rejects a release without both supported platforms", (context) => {
  const directory = createRelease(context, (manifest) => {
    delete manifest.platforms["linux-x86_64"];
  });

  assert.throws(
    () => validateReleaseAssets({ directory, repository: REPOSITORY, tag: TAG }),
    /missing a linux updater entry/,
  );
});

function createRelease(context, alterManifest = () => {}) {
  const directory = mkdtempSync(join(tmpdir(), "baudbound-release-test-"));
  context.after(() => rmSync(directory, { force: true, recursive: true }));

  const windows = "BaudBound_2.0.0_x64-setup.exe";
  const linux = "BaudBound_2.0.0_amd64.AppImage";
  const windowsSignature = "windows-signature";
  const linuxSignature = "linux-signature";
  const manifest = {
    version: "2.0.0",
    notes: "Production release",
    pub_date: "2026-07-12T12:00:00Z",
    platforms: {
      "windows-x86_64": releaseEntry(windows, windowsSignature),
      "linux-x86_64": releaseEntry(linux, linuxSignature),
    },
  };
  alterManifest(manifest);

  write(directory, windows, "windows-installer");
  write(directory, `${windows}.sig`, windowsSignature);
  write(directory, linux, "linux-appimage");
  write(directory, `${linux}.sig`, linuxSignature);
  write(directory, "latest.json", JSON.stringify(manifest));
  return directory;
}

function releaseEntry(asset, signature) {
  return {
    signature,
    url: `https://github.com/${REPOSITORY}/releases/download/${TAG}/${asset}`,
  };
}

function write(directory, name, contents) {
  writeFileSync(join(directory, name), contents, "utf8");
}

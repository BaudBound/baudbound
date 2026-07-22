import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import assert from "node:assert/strict";
import { validateReleaseAssets } from "./release-assets.mjs";
import { checksumManifest } from "./release-checksums.mjs";

const TAG = "v2.0.0";
const REPOSITORY = "NATroutter/BaudBound";

test("accepts matching Windows and Linux updater artifacts", (context) => {
  const directory = createRelease(context);
  const result = validateReleaseAssets({ directory, repository: REPOSITORY, tag: TAG });

  assert.deepEqual(result.platforms, ["linux-x86_64", "windows-x86_64"]);
  assert.equal(result.version, "2.0.0");
});

test("accepts Tauri GitHub API URLs that match release asset metadata", (context) => {
  const releaseAssets = [];
  const directory = createRelease(context, (manifest) => {
    setApiUrl(manifest, releaseAssets, "windows-x86_64", 1001);
    setApiUrl(manifest, releaseAssets, "linux-x86_64", 1002);
  });
  const result = validateReleaseAssets({
    directory,
    releaseAssets,
    repository: REPOSITORY,
    tag: TAG,
  });

  assert.deepEqual(result.platforms, ["linux-x86_64", "windows-x86_64"]);
});

test("rejects a Tauri API URL that is absent from release asset metadata", (context) => {
  const directory = createRelease(context, (manifest) => {
    manifest.platforms["linux-x86_64"].url = apiUrl(1002);
  });

  assert.throws(
    () => validateReleaseAssets({ directory, releaseAssets: [], repository: REPOSITORY, tag: TAG }),
    /does not match an uploaded release asset/,
  );
});

test("rejects a Tauri API URL for another repository", (context) => {
  const releaseAssets = [];
  const directory = createRelease(context, (manifest) => {
    const url = "https://api.github.com/repos/another/project/releases/assets/1002";
    manifest.platforms["linux-x86_64"].url = url;
    releaseAssets.push({ apiUrl: url, name: "BaudBound_2.0.0_amd64.AppImage" });
  });

  assert.throws(
    () => validateReleaseAssets({ directory, releaseAssets, repository: REPOSITORY, tag: TAG }),
    /API URL must target NATroutter\/BaudBound/,
  );
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

test("rejects a release without a Debian package", (context) => {
  const directory = createRelease(context);
  rmSync(join(directory, "BaudBound_2.0.0_amd64.deb"));

  assert.throws(
    () => validateReleaseAssets({ directory, repository: REPOSITORY, tag: TAG }),
    /exactly one Linux Debian package/,
  );
});

test("rejects a release without an RPM package", (context) => {
  const directory = createRelease(context);
  rmSync(join(directory, "BaudBound-2.0.0-1.x86_64.rpm"));

  assert.throws(
    () => validateReleaseAssets({ directory, repository: REPOSITORY, tag: TAG }),
    /exactly one Linux RPM package/,
  );
});

test("rejects a native package with the wrong version", (context) => {
  const directory = createRelease(context);
  rmSync(join(directory, "BaudBound_2.0.0_amd64.deb"));
  write(directory, "BaudBound_1.9.0_amd64.deb", "linux-deb");

  assert.throws(
    () => validateReleaseAssets({ directory, repository: REPOSITORY, tag: TAG }),
    /Debian package filename does not contain version 2\.0\.0/,
  );
});

test("rejects a modified installer", (context) => {
  const directory = createRelease(context);
  write(directory, "BaudBound_2.0.0_amd64.deb", "modified-linux-deb");

  assert.throws(
    () => validateReleaseAssets({ directory, repository: REPOSITORY, tag: TAG }),
    /checksum does not match SHA256SUMS/,
  );
});

test("rejects a release without checksums", (context) => {
  const directory = createRelease(context);
  rmSync(join(directory, "SHA256SUMS"));

  assert.throws(
    () => validateReleaseAssets({ directory, repository: REPOSITORY, tag: TAG }),
    /release is missing SHA256SUMS/,
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
  write(directory, "BaudBound_2.0.0_amd64.deb", "linux-deb");
  write(directory, "BaudBound-2.0.0-1.x86_64.rpm", "linux-rpm");
  write(directory, "latest.json", JSON.stringify(manifest));
  const assets = new Map(
    [windows, linux, "BaudBound_2.0.0_amd64.deb", "BaudBound-2.0.0-1.x86_64.rpm"]
      .map((name) => [name, join(directory, name)]),
  );
  write(directory, "SHA256SUMS", checksumManifest(assets));
  return directory;
}

function releaseEntry(asset, signature) {
  return {
    signature,
    url: `https://github.com/${REPOSITORY}/releases/download/${TAG}/${asset}`,
  };
}

function setApiUrl(manifest, releaseAssets, platform, assetId) {
  const entry = manifest.platforms[platform];
  const name = decodeURIComponent(entry.url.split("/").at(-1));
  entry.url = apiUrl(assetId);
  releaseAssets.push({ apiUrl: entry.url, name });
}

function apiUrl(assetId) {
  return `https://api.github.com/repos/${REPOSITORY}/releases/assets/${assetId}`;
}

function write(directory, name, contents) {
  writeFileSync(join(directory, name), contents, "utf8");
}

import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import assert from "node:assert/strict";
import { buildUpdaterManifest } from "./build-updater-manifest.mjs";
import { validateReleaseAssets } from "./release-assets.mjs";
import { checksumManifest } from "./release-checksums.mjs";

const TAG = "v2.0.0";
const REPOSITORY = "BaudBound/baudbound";
const PUBLISHED_AT = "2026-07-12T12:00:00Z";

test("composes one entry per declared updater key", (context) => {
  const directory = createUpdaterInputs(context);
  const manifest = buildUpdaterManifest({
    directory,
    notes: "Production release",
    publishedAt: PUBLISHED_AT,
    repository: REPOSITORY,
    tag: TAG,
  });

  assert.equal(manifest.version, "2.0.0");
  assert.deepEqual(Object.keys(manifest.platforms).sort(), [
    "linux-aarch64",
    "linux-x86_64",
    "windows-x86_64",
  ]);
  assert.equal(manifest.platforms["linux-aarch64"].signature, "linux-arm-signature");
  assert.equal(
    manifest.platforms["linux-aarch64"].url,
    `https://github.com/${REPOSITORY}/releases/download/${TAG}/Baudbound_2.0.0_aarch64.AppImage`,
  );
  assert.equal(manifest.pub_date, PUBLISHED_AT);
});

test("does not confuse one architecture's AppImage for another", (context) => {
  const directory = createUpdaterInputs(context);
  const manifest = buildUpdaterManifest({
    directory,
    notes: "",
    publishedAt: PUBLISHED_AT,
    repository: REPOSITORY,
    tag: TAG,
  });

  assert.ok(manifest.platforms["linux-x86_64"].url.endsWith("Baudbound_2.0.0_amd64.AppImage"));
  assert.equal(manifest.platforms["linux-x86_64"].signature, "linux-x86-signature");
});

test("fails when a declared platform has no updater payload", (context) => {
  const directory = createUpdaterInputs(context, ["Baudbound_2.0.0_aarch64.AppImage"]);

  assert.throws(
    () =>
      buildUpdaterManifest({
        directory,
        notes: "",
        publishedAt: PUBLISHED_AT,
        repository: REPOSITORY,
        tag: TAG,
      }),
    /no updater payload for linux-aarch64/,
  );
});

test("fails when a payload has no signature beside it", (context) => {
  const directory = createUpdaterInputs(context);
  rmSync(join(directory, "Baudbound_2.0.0_aarch64.AppImage.sig"));

  assert.throws(
    () =>
      buildUpdaterManifest({
        directory,
        notes: "",
        publishedAt: PUBLISHED_AT,
        repository: REPOSITORY,
        tag: TAG,
      }),
    /no signature for Baudbound_2\.0\.0_aarch64\.AppImage/,
  );
});

test("rejects a tag that is not a release version", (context) => {
  const directory = createUpdaterInputs(context);

  assert.throws(
    () =>
      buildUpdaterManifest({
        directory,
        notes: "",
        publishedAt: PUBLISHED_AT,
        repository: REPOSITORY,
        tag: "release-2",
      }),
    /invalid release tag release-2/,
  );
});

// Generation and verification are deliberately separate implementations, so
// nothing but a test makes them agree. This is the one that would catch the
// two drifting apart before a release does.
test("produces a manifest the independent release verifier accepts", (context) => {
  const directory = createUpdaterInputs(context);
  const packages = [
    "Baudbound_2.0.0_amd64.deb",
    "Baudbound-2.0.0-1.x86_64.rpm",
    "Baudbound_2.0.0_arm64.deb",
    "Baudbound-2.0.0-1.aarch64.rpm",
  ];
  for (const name of packages) {
    writeFileSync(join(directory, name), `payload-${name}`, "utf8");
  }

  const manifest = buildUpdaterManifest({
    directory,
    notes: "Production release",
    publishedAt: PUBLISHED_AT,
    repository: REPOSITORY,
    tag: TAG,
  });
  writeFileSync(join(directory, "latest.json"), JSON.stringify(manifest, null, 2), "utf8");

  const assets = new Map(
    [
      "BaudBound_2.0.0_x64-setup.exe",
      "Baudbound_2.0.0_amd64.AppImage",
      "Baudbound_2.0.0_aarch64.AppImage",
      ...packages,
    ].map((name) => [name, join(directory, name)]),
  );
  writeFileSync(join(directory, "SHA256SUMS"), checksumManifest(assets, "2.0.0"), "utf8");

  const result = validateReleaseAssets({ directory, repository: REPOSITORY, tag: TAG });
  assert.deepEqual(result.platforms, ["linux-aarch64", "linux-x86_64", "windows-x86_64"]);
});

function createUpdaterInputs(context, omit = []) {
  const directory = mkdtempSync(join(tmpdir(), "baudbound-updater-test-"));
  context.after(() => rmSync(directory, { force: true, recursive: true }));

  const payloads = {
    "BaudBound_2.0.0_x64-setup.exe": "windows-signature",
    "Baudbound_2.0.0_amd64.AppImage": "linux-x86-signature",
    "Baudbound_2.0.0_aarch64.AppImage": "linux-arm-signature",
  };

  for (const [name, signature] of Object.entries(payloads)) {
    if (omit.includes(name)) continue;
    writeFileSync(join(directory, name), `payload-${name}`, "utf8");
    writeFileSync(join(directory, `${name}.sig`), signature, "utf8");
  }
  return directory;
}

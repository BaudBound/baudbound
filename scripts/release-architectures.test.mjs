import test from "node:test";
import assert from "node:assert/strict";
import {
  RELEASE_ARCHITECTURES,
  expectedArtifacts,
  expectedLinuxArtifacts,
  updaterKeys,
} from "./release-architectures.mjs";

test("declares both supported Linux architectures", () => {
  assert.deepEqual(
    RELEASE_ARCHITECTURES.map((entry) => entry.id),
    ["x86_64", "aarch64"],
  );
});

test("expects three Linux artifacts for each architecture", () => {
  const artifacts = expectedLinuxArtifacts("2.0.0");
  assert.equal(artifacts.length, 6);
  assert.deepEqual(
    artifacts.filter((artifact) => artifact.format === "deb").map((artifact) => artifact.label),
    ["Linux Debian package (x86_64)", "Linux Debian package (aarch64)"],
  );
});

test("matches the x86_64 artifact names the existing release produces", () => {
  const artifacts = expectedLinuxArtifacts("2.0.0");
  const match = (label, name) =>
    artifacts.find((artifact) => artifact.label === label).matches(name);

  assert.ok(match("Linux Debian package (x86_64)", "Baudbound_2.0.0_amd64.deb"));
  assert.ok(match("Linux RPM package (x86_64)", "Baudbound-2.0.0-1.x86_64.rpm"));
  assert.ok(match("Linux AppImage (x86_64)", "Baudbound_2.0.0_amd64.AppImage"));
});

test("does not match one architecture's artifact against another", () => {
  const artifacts = expectedLinuxArtifacts("2.0.0");
  const debianArm = artifacts.find(
    (artifact) => artifact.label === "Linux Debian package (aarch64)",
  );

  assert.ok(debianArm.matches("Baudbound_2.0.0_arm64.deb"));
  assert.ok(!debianArm.matches("Baudbound_2.0.0_amd64.deb"));
});

test("includes the Windows installer in the full artifact set", () => {
  const labels = expectedArtifacts("2.0.0").map((artifact) => artifact.label);
  assert.ok(labels.includes("Windows NSIS installer"));
  assert.equal(labels.length, 7);
});

test("lists every declared updater key", () => {
  assert.deepEqual(updaterKeys(), ["windows-x86_64", "linux-x86_64", "linux-aarch64"]);
});

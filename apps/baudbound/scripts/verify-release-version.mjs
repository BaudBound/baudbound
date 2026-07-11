import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = resolve(appRoot, "../..");
const tag = process.argv[2];

if (!tag || !/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(tag)) {
  fail(`release tag must use vMAJOR.MINOR.PATCH syntax, found ${JSON.stringify(tag)}`);
}

const expectedVersion = tag.slice(1);
const metadata = JSON.parse(
  execFileSync("cargo", ["metadata", "--format-version", "1", "--no-deps"], {
    cwd: repositoryRoot,
    encoding: "utf8",
  }),
);
const runnerPackage = metadata.packages.find((candidate) => candidate.name === "baudbound");
const tauriConfig = readJson(resolve(appRoot, "tauri.conf.json"));
const uiPackage = readJson(resolve(appRoot, "ui/package.json"));

const versions = [
  ["Cargo package", runnerPackage?.version],
  ["Tauri config", tauriConfig.version],
  ["desktop UI package", uiPackage.version],
];

for (const [source, version] of versions) {
  if (version !== expectedVersion) {
    fail(`${source} version ${JSON.stringify(version)} does not match tag ${tag}`);
  }
}

console.log(`Release versions agree on ${expectedVersion}.`);

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function fail(message) {
  console.error(message);
  process.exit(1);
}

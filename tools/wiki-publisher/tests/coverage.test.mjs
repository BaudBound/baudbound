import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import { validateDocumentationCoverage } from "../src/coverage.mjs";

test("validates source-derived node, desktop, CLI, config, and repository coverage", async (context) => {
  const root = await fixtureRoot(context);
  await files(root, {
    "nodes/example.ts": 'export const node = { actionType: "action.example" };',
    "ui.tsx": `{
      icon: Gauge,
      id: "dashboard",
      label: "Dashboard",
    }`,
    "cli.rs": rustCliFixture(),
    "config.rs": rustConfigFixture(),
    "required.txt": "present",
  });
  const manifestPath = path.join(root, "coverage.json");
  await writeFile(manifestPath, JSON.stringify(manifest()), "utf8");
  const pages = documentationPages();

  const result = await validateDocumentationCoverage(root, pages, manifestPath);

  assert.deepEqual(result, { pages: 4, paths: 1 });
});

test("reports missing source-derived documentation", async (context) => {
  const root = await fixtureRoot(context);
  await files(root, {
    "nodes/example.ts": 'export const node = { actionType: "action.example" };',
    "ui.tsx": '{ icon: Gauge, id: "dashboard", label: "Dashboard" }',
    "cli.rs": rustCliFixture(),
    "config.rs": rustConfigFixture(),
  });
  const manifestPath = path.join(root, "coverage.json");
  await writeFile(manifestPath, JSON.stringify(manifest()), "utf8");

  await assert.rejects(
    validateDocumentationCoverage(root, documentationPages().map((page) => ({ ...page, content: "" })), manifestPath),
      /action\.example[\s\S]*desktop tab marker[\s\S]*CLI command status[\s\S]*config field runner\.trigger_reload_seconds/,
  );
});

test("rejects stale duplicate and unparseable desktop navigation contracts", async (context) => {
  const root = await fixtureRoot(context);
  await files(root, {
    "nodes/example.ts": 'export const node = { actionType: "action.example" };',
    "ui.tsx": `
      { label: "Dashboard", id: "dashboard", icon: Gauge }
      { icon: Gauge, id: "dashboard", label: "Duplicate" }
    `,
    "cli.rs": rustCliFixture(),
    "config.rs": rustConfigFixture(),
    "required.txt": "present",
  });
  const manifestPath = path.join(root, "coverage.json");
  await writeFile(manifestPath, JSON.stringify(manifest()), "utf8");
  const pages = documentationPages().map((page) =>
    page.path === "desktop"
      ? { ...page, content: "<!-- desktop-tab:dashboard -->\n<!-- desktop-tab:dashboard -->\n<!-- desktop-tab:settings -->" }
      : page,
  );

  await assert.rejects(
    validateDocumentationCoverage(root, pages, manifestPath),
    /duplicate desktop navigation id dashboard[\s\S]*duplicate desktop tab marker dashboard[\s\S]*removed or unknown desktop tab settings/,
  );

  await writeFile(path.join(root, "ui.tsx"), "export const navigation = [];", "utf8");
  await assert.rejects(
    validateDocumentationCoverage(root, documentationPages(), manifestPath),
    /does not contain any recognizable desktop navigation items[\s\S]*removed or unknown desktop tab dashboard/,
  );
});

function manifest() {
  return {
    requiredPages: ["nodes", "desktop", "cli", "config"],
    requiredPaths: ["required.txt"],
    nodes: { sources: "nodes/*.ts", page: "nodes", expectedCount: 1 },
    desktop: { source: "ui.tsx", page: "desktop" },
    cli: { source: "cli.rs", page: "cli" },
    configuration: {
      source: "config.rs",
      page: "config",
      structs: ["RunnerSettings"],
      sections: { runner: "RunnerSettings" },
    },
  };
}

function documentationPages() {
  return [
    { path: "nodes", content: "`action.example`" },
    { path: "desktop", content: "<!-- desktop-tab:dashboard -->" },
    { path: "cli", content: "baudbound status `--json`" },
    { path: "config", content: "`runner.trigger_reload_seconds`" },
  ];
}

function rustCliFixture() {
  return `
pub enum Command {
    Status {
        #[arg(long)]
        json: bool,
    },
}
pub enum ConfigCommand {}
pub enum ScriptCommand {}
pub enum HotkeyCommand {}
pub enum SecretCommand {}
`;
}

function rustConfigFixture() {
  return `pub struct RunnerSettings {\n    pub trigger_reload_seconds: u64,\n}`;
}

async function fixtureRoot(context) {
  const root = path.join(tmpdir(), `baudbound-wiki-coverage-${crypto.randomUUID()}`);
  await mkdir(root, { recursive: true });
  context.after(async () => {
    const { rm } = await import("node:fs/promises");
    await rm(root, { force: true, recursive: true });
  });
  return root;
}

async function files(root, entries) {
  for (const [relativePath, content] of Object.entries(entries)) {
    const destination = path.join(root, relativePath);
    await mkdir(path.dirname(destination), { recursive: true });
    await writeFile(destination, content, "utf8");
  }
}

import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import path from "node:path";

import fg from "fast-glob";

export async function validateDocumentationCoverage(repositoryRoot, pages, manifestPath) {
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  const pageByPath = new Map(pages.map((page) => [page.path, page]));
  const errors = [];

  checkRequiredPages(manifest.requiredPages, pageByPath, errors);
  checkExternalHosts(manifest.approvedExternalHosts ?? [], pages, errors);
  await checkRequiredPaths(repositoryRoot, manifest.requiredPaths, errors);
  await checkNodeCoverage(repositoryRoot, manifest.nodes, pageByPath, errors);
  await checkDesktopCoverage(repositoryRoot, manifest.desktop, pageByPath, errors);
  await checkCliCoverage(repositoryRoot, manifest.cli, pageByPath, errors);
  await checkConfigCoverage(repositoryRoot, manifest.configuration, pageByPath, errors);
  if (manifest.security) {
    await checkSecurityCoverage(repositoryRoot, manifest.security, pageByPath, errors);
  }

  if (errors.length > 0) {
    throw new Error(`documentation coverage failed:\n${errors.map((error) => `- ${error}`).join("\n")}`);
  }

  return {
    pages: manifest.requiredPages.length,
    paths: manifest.requiredPaths.length,
  };
}

function checkExternalHosts(approvedHosts, pages, errors) {
  const approved = new Set(approvedHosts);
  for (const page of pages) {
    for (const match of page.content.matchAll(/\]\((https:\/\/[^)\s]+)(?:\s+"[^"]*")?\)/g)) {
      const host = new URL(match[1]).hostname.toLowerCase();
      if (!approved.has(host)) {
        errors.push(`${page.path} links to unapproved external host ${host}`);
      }
    }
  }
}

function checkRequiredPages(requiredPages, pageByPath, errors) {
  for (const pagePath of requiredPages) {
    if (!pageByPath.has(pagePath)) errors.push(`required wiki page is missing: ${pagePath}`);
  }
}

async function checkRequiredPaths(repositoryRoot, requiredPaths, errors) {
  for (const requiredPath of requiredPaths) {
    const matches = await fg(requiredPath, { cwd: repositoryRoot, onlyFiles: false });
    if (matches.length === 0) errors.push(`documented repository surface is missing: ${requiredPath}`);
  }
}

async function checkNodeCoverage(repositoryRoot, definition, pageByPath, errors) {
  const files = (await fg(definition.sources, { cwd: repositoryRoot, onlyFiles: true })).sort();
  const actionTypes = new Set();
  const fingerprint = createHash("sha256");
  for (const file of files) {
    const source = await readFile(path.join(repositoryRoot, file), "utf8");
    fingerprint.update(file);
    fingerprint.update("\0");
    fingerprint.update(source);
    fingerprint.update("\0");
    const match = source.match(/\bactionType:\s*["']([^"']+)["']/);
    if (match) actionTypes.add(match[1]);
  }
  const content = requiredPageContent(definition.page, pageByPath, errors);
  for (const actionType of [...actionTypes].sort()) {
    if (!content.includes(`\`${actionType}\``)) {
      errors.push(`${definition.page} does not document node ${actionType}`);
    }
  }
  if (actionTypes.size !== definition.expectedCount) {
    errors.push(`node registry has ${actionTypes.size} definitions; coverage baseline is ${definition.expectedCount}`);
  }
  const actualFingerprint = fingerprint.digest("hex");
  if (definition.sourceFingerprint && actualFingerprint !== definition.sourceFingerprint) {
    errors.push(
      `node definition contract changed (${actualFingerprint}); review fields and outputs in ${definition.page}, then update its coverage fingerprint`,
    );
  }
}

async function checkDesktopCoverage(repositoryRoot, definition, pageByPath, errors) {
  const source = await readFile(path.join(repositoryRoot, definition.source), "utf8");
  const items = desktopNavigationItems(source);
  const content = requiredPageContent(definition.page, pageByPath, errors);
  const sourceIds = new Set();
  for (const { id, label } of items) {
    if (sourceIds.has(id)) {
      errors.push(`${definition.source} contains duplicate desktop navigation id ${id}`);
      continue;
    }
    sourceIds.add(id);
    if (!content.includes(`<!-- desktop-tab:${id} -->`)) {
      errors.push(`${definition.page} is missing the desktop tab marker for ${id} (${label})`);
    }
  }

  if (items.length === 0) {
    errors.push(`${definition.source} does not contain any recognizable desktop navigation items`);
  }

  const documentedIds = [...content.matchAll(/<!--\s*desktop-tab:([a-z0-9-]+)\s*-->/g)].map(
    (match) => match[1],
  );
  const seenDocumentedIds = new Set();
  for (const id of documentedIds) {
    if (seenDocumentedIds.has(id)) {
      errors.push(`${definition.page} contains duplicate desktop tab marker ${id}`);
    } else if (!sourceIds.has(id)) {
      errors.push(`${definition.page} documents removed or unknown desktop tab ${id}`);
    }
    seenDocumentedIds.add(id);
  }
}

function desktopNavigationItems(source) {
  const items = [];
  for (const match of source.matchAll(/\{[^{}]*\}/gs)) {
    const body = match[0];
    if (!/\bicon\s*:/.test(body)) continue;
    const id = body.match(/\bid\s*:\s*"([^"]+)"/)?.[1];
    const label = body.match(/\blabel\s*:\s*"([^"]+)"/)?.[1];
    if (id && label) items.push({ id, label });
  }
  return items;
}

async function checkCliCoverage(repositoryRoot, definition, pageByPath, errors) {
  const source = await readFile(path.join(repositoryRoot, definition.source), "utf8");
  const content = requiredPageContent(definition.page, pageByPath, errors);
  const enumNames = ["Command", "ConfigCommand", "ScriptCommand", "HotkeyCommand", "SecretCommand"];
  const commands = [];
  for (const enumName of enumNames) {
    const body = rustEnumBody(source, enumName);
    for (const match of body.matchAll(/^    ([A-Z][A-Za-z0-9]+)(?:\s*[,{])/gm)) {
      commands.push(toKebabCase(match[1]));
    }
  }
  for (const command of new Set(commands)) {
    if (!new RegExp(`(?:baudbound\\s+|script\\s+|config\\s+|hotkey\\s+|secret\\s+|\\b)${escapeRegex(command)}\\b`).test(content)) {
      errors.push(`${definition.page} does not document CLI command ${command}`);
    }
  }
  const options = [...source.matchAll(/#\[arg\(long[^\]]*\)\]\s*\n\s*pub\s+([a-z0-9_]+):/g)].map(
    (match) => `--${match[1].replaceAll("_", "-")}`,
  );
  for (const option of new Set(options)) {
    if (!content.includes(`\`${option}\``) && !content.includes(option)) {
      errors.push(`${definition.page} does not document CLI option ${option}`);
    }
  }
}

async function checkConfigCoverage(repositoryRoot, definition, pageByPath, errors) {
  const source = await readFile(path.join(repositoryRoot, definition.source), "utf8");
  const content = requiredPageContent(definition.page, pageByPath, errors);
  const structFields = new Map();
  for (const structName of definition.structs) {
    const body = rustStructBody(source, structName);
    structFields.set(
      structName,
      [...body.matchAll(/^    pub ([a-z0-9_]+):/gm)].map((match) => match[1]),
    );
  }
  for (const [prefix, structName] of Object.entries(definition.sections)) {
    for (const field of structFields.get(structName) ?? []) {
      const key = prefix === "serial.devices.*" ? field : `${prefix}.${field}`;
      const documented = content.includes(`\`${key}\``) ||
        (prefix === "serial.devices.*" && content.includes(`\`${field}\``));
      if (!documented) errors.push(`${definition.page} does not document config field ${key}`);
    }
  }
}

async function checkSecurityCoverage(repositoryRoot, definition, pageByPath, errors) {
  const capabilityContract = JSON.parse(
    await readFile(path.join(repositoryRoot, definition.capabilitySource), "utf8"),
  );
  const permissionSource = await readFile(
    path.join(repositoryRoot, definition.permissionSource),
    "utf8",
  );
  const content = requiredPageContent(definition.page, pageByPath, errors);
  const capabilities = new Set(Object.values(capabilityContract.nodes).flat());
  capabilities.add("runtime.persistent_storage");
  capabilities.add("runtime.secrets");
  const permissions = new Set(
    [...permissionSource.matchAll(/=>\s*\("([a-z0-9_]+)",\s*RiskLevel::/g)].map(
      (match) => match[1],
    ),
  );
  for (const match of permissionSource.matchAll(/name:\s*"([a-z0-9_]+)"\.to_owned\(\)/g)) {
    permissions.add(match[1]);
  }
  for (const value of [...capabilities, ...permissions].sort()) {
    if (!content.includes(`\`${value}\``)) {
      errors.push(`${definition.page} does not document security value ${value}`);
    }
  }
}

function requiredPageContent(pagePath, pageByPath, errors) {
  const page = pageByPath.get(pagePath);
  if (page) return page.content;
  errors.push(`coverage target page is missing: ${pagePath}`);
  return "";
}

function rustEnumBody(source, name) {
  return rustItemBody(source, `pub enum ${name}`);
}

function rustStructBody(source, name) {
  return rustItemBody(source, `pub struct ${name}`);
}

function rustItemBody(source, signature) {
  const start = source.indexOf(signature);
  if (start === -1) throw new Error(`could not find Rust item ${signature}`);
  const open = source.indexOf("{", start);
  let depth = 0;
  for (let index = open; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") depth -= 1;
    if (depth === 0) return source.slice(open + 1, index);
  }
  throw new Error(`could not parse Rust item ${signature}`);
}

function toKebabCase(value) {
  return value.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase();
}

function escapeRegex(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

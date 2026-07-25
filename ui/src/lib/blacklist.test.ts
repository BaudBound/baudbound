import { describe, expect, it } from "vitest";

import {
  blacklistBlocksUpdateSource,
  blacklistEntriesForRepositoryScript,
  blacklistEntriesForUrl,
} from "@/lib/blacklist";
import type {
  BlacklistEntry,
  RepositoryScriptSummary,
  ScriptStatus,
} from "@/lib/runner-api";

describe("blacklistEntriesForUrl", () => {
  it("matches exact domains without matching lookalikes or subdomains by default", () => {
    const entry = blacklistEntry("domain", "malicious.example");

    expect(blacklistEntriesForUrl([entry], "https://malicious.example/file.bbs")).toEqual([
      entry,
    ]);
    expect(
      blacklistEntriesForUrl([entry], "https://files.malicious.example/file.bbs"),
    ).toEqual([]);
    expect(
      blacklistEntriesForUrl([entry], "https://malicious.example.attacker.net/file.bbs"),
    ).toEqual([]);
  });

  it("matches descendant hosts only when subdomain matching is enabled", () => {
    const entry = {
      ...blacklistEntry("domain", "malicious.example"),
      subdomains: true,
    };

    expect(
      blacklistEntriesForUrl([entry], "https://deep.files.malicious.example/file.bbs"),
    ).toEqual([entry]);
  });

  it.each([
    "https://github.com/BadActor/repository/raw/master/repository.json",
    "https://raw.githubusercontent.com/BadActor/repository/master/repository.json",
    "https://api.github.com/repos/BadActor/repository",
    "https://badactor.github.io/repository/repository.json",
    "https://gist.github.com/BadActor/012345",
    "https://gist.githubusercontent.com/BadActor/012345/raw/file.bbs",
  ])("recognizes a GitHub publisher from %s", (url) => {
    const entry = blacklistEntry("publisher", "github:badactor");

    expect(blacklistEntriesForUrl([entry], url)).toEqual([entry]);
  });
});

describe("blacklistEntriesForRepositoryScript", () => {
  it("combines repository, script, and package advisories", () => {
    const repository = blacklistEntry(
      "repository",
      "https://example.com/repository.json",
    );
    const script = blacklistEntry("script", "script-1");
    const packageEntry = blacklistEntry("package", "abc123");

    expect(
      blacklistEntriesForRepositoryScript(
        [repository, script, packageEntry],
        repositoryScript(),
      ),
    ).toEqual([repository, script, packageEntry]);
  });
});

describe("blacklistBlocksUpdateSource", () => {
  it("allows an exact package restriction to discover a safe replacement", () => {
    const status = scriptStatus([blacklistEntry("package", "old-hash", "high")]);

    expect(blacklistBlocksUpdateSource(status)).toBe(false);
  });

  it("blocks update checks when the trusted repository is restricted", () => {
    const status = scriptStatus([
      blacklistEntry(
        "repository",
        "https://example.com/repository.json",
        "medium",
      ),
    ]);

    expect(blacklistBlocksUpdateSource(status)).toBe(true);
  });
});

function blacklistEntry(
  scope: BlacklistEntry["scope"],
  target: string,
  severity: BlacklistEntry["severity"] = "low",
): BlacklistEntry {
  return {
    advisory_url: "https://example.com/advisory",
    id: `${scope}-${target}`,
    published_at: "2026-07-25T00:00:00Z",
    reason: "Test reason",
    scope,
    severity,
    subdomains: false,
    target,
    title: "Test advisory",
    updated: "2026-07-25T00:00:00Z",
  };
}

function repositoryScript(): RepositoryScriptSummary {
  return {
    author: "Tester",
    information_mismatch: null,
    information_mismatch_refresh_required: false,
    installed: false,
    minimum_runner_version: "2.0.0",
    name: "Test script",
    official: false,
    package_hash: "abc123",
    published_at: "2026-07-25T00:00:00Z",
    repository_name: "Test repository",
    repository_url: "https://example.com/repository.json",
    risk_level: "low",
    script_id: "script-1",
    summary: "Test script",
    target_runtimes: ["Linux Desktop"],
    version: "1.0.0",
  };
}

function scriptStatus(entries: BlacklistEntry[]): ScriptStatus {
  return {
    blacklist: { entries, severity: entries[0]?.severity ?? null },
  } as ScriptStatus;
}

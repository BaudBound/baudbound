import type {
  BlacklistEntry,
  BlacklistSeverity,
  RepositoryScriptRecord,
  RepositoryScriptSummary,
  ScriptStatus,
} from "@/lib/runner-api";

export function blacklistLabel(severity: BlacklistSeverity | null) {
  switch (severity) {
    case "low":
      return "Advisory";
    case "medium":
      return "Restricted";
    case "high":
      return "Quarantined";
    case "critical":
      return "Critical quarantine";
    default:
      return null;
  }
}

export function blacklistVariant(severity: BlacklistSeverity | null) {
  if (severity === "critical" || severity === "high") {
    return "destructive" as const;
  }
  if (severity === "medium" || severity === "low") {
    return "medium" as const;
  }
  return "muted" as const;
}

export function blacklistBlocksDistribution(script: ScriptStatus) {
  return (
    script.blacklist.severity === "medium" ||
    script.blacklist.severity === "high" ||
    script.blacklist.severity === "critical"
  );
}

export function blacklistBlocksExecution(script: ScriptStatus) {
  return (
    script.blacklist.severity === "high" ||
    script.blacklist.severity === "critical"
  );
}

export function blacklistBlocksUpdateSource(script: ScriptStatus) {
  return script.blacklist.entries.some(
    (entry) =>
      entry.scope !== "package" &&
      (entry.severity === "medium" ||
        entry.severity === "high" ||
        entry.severity === "critical"),
  );
}

export function blacklistEntrySummary(entry: BlacklistEntry) {
  return `${entry.title}: ${entry.reason}`;
}

export function blacklistEntriesForUrl(
  entries: BlacklistEntry[],
  value: string,
) {
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    return [];
  }
  const host = url.hostname.toLowerCase().replace(/\.$/, "");
  const repository = normalizeUrl(url);
  const publisher = githubPublisher(url);
  return entries.filter((entry) => {
    if (entry.scope === "repository") {
      try {
        return normalizeUrl(new URL(entry.target)) === repository;
      } catch {
        return false;
      }
    }
    if (entry.scope === "domain") {
      const target = entry.target.toLowerCase();
      return host === target || (entry.subdomains && host.endsWith(`.${target}`));
    }
    return entry.scope === "publisher" && publisher === entry.target;
  });
}

export function blacklistEntriesForRepositoryScript(
  entries: BlacklistEntry[],
  script: RepositoryScriptSummary | RepositoryScriptRecord,
) {
  const packageHash =
    "package_hash" in script ? script.package_hash : script.entry.latest.sha256;
  return entries.filter(
    (entry) =>
      (entry.scope === "script" && entry.target === script.script_id) ||
      (entry.scope === "package" &&
        entry.target.toLowerCase() === packageHash.toLowerCase()) ||
      blacklistEntriesForUrl([entry], script.repository_url).length > 0,
  );
}

function normalizeUrl(url: URL) {
  const normalized = new URL(url);
  normalized.hash = "";
  if (normalized.protocol === "https:" && normalized.port === "443") {
    normalized.port = "";
  }
  normalized.hostname = normalized.hostname.toLowerCase().replace(/\.$/, "");
  return normalized.toString();
}

function githubPublisher(url: URL) {
  const segments = url.pathname.split("/").filter(Boolean);
  const host = url.hostname.toLowerCase();
  let owner: string | undefined;
  if (
    host === "github.com" ||
    host === "raw.githubusercontent.com" ||
    host === "gist.github.com" ||
    host === "gist.githubusercontent.com"
  ) {
    owner = segments[0];
  } else if (host === "api.github.com" && segments[0] === "repos") {
    owner = segments[1];
  } else if (host.endsWith(".github.io")) {
    owner = host.slice(0, -".github.io".length);
  }
  return owner ? `github:${owner.toLowerCase()}` : null;
}

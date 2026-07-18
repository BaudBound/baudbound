import type { RunLogEntry, StoredRunRecord } from "@/lib/runner-api";

export type VariableRow = {
  name: string;
  preview: string;
  raw: unknown;
  type: string;
};

export function countLogsByLevel(logs: RunLogEntry[]) {
  return logs.reduce<Record<string, number>>((counts, log) => {
    counts[log.level] = (counts[log.level] ?? 0) + 1;
    return counts;
  }, {});
}

export function logLevels(logs: RunLogEntry[]) {
  return Object.keys(countLogsByLevel(logs)).sort();
}

export function filterLogs(
  logs: RunLogEntry[],
  options: { level: string; query: string },
) {
  const query = options.query.trim().toLowerCase();
  return logs.filter((log) => {
    if (options.level !== "all" && log.level !== options.level) {
      return false;
    }
    if (!query) {
      return true;
    }
    return [log.level, log.action_type ?? "", log.node_id ?? "", log.message]
      .join("\n")
      .toLowerCase()
      .includes(query);
  });
}

export function nodeActionType(logs: RunLogEntry[], nodeId: string) {
  return logs.find((log) => log.node_id === nodeId && log.action_type)?.action_type ?? null;
}

export function runHasErrors(run: StoredRunRecord) {
  return run.status === "failed" || run.logs.some((log) => log.level === "error");
}

export function runSummary(runs: StoredRunRecord[]) {
  return {
    completed: runs.filter((run) => run.status === "completed").length,
    failed: runs.filter((run) => run.status === "failed").length,
    cancelled: runs.filter((run) => run.status === "cancelled").length,
    withErrors: runs.filter(runHasErrors).length,
  };
}

export function runStatusVariant(status: StoredRunRecord["status"]) {
  if (status === "completed") return "good" as const;
  return status === "cancelled" ? ("medium" as const) : ("destructive" as const);
}

export function variableRows(variables: Record<string, unknown>): VariableRow[] {
  return Object.entries(variables)
    .map(([name, value]) => ({
      name,
      preview: previewValue(value),
      raw: value,
      type: valueType(value),
    }))
    .sort((left, right) => left.name.localeCompare(right.name));
}

export function filterVariables(rows: VariableRow[], query: string) {
  const normalized = query.trim().toLowerCase();
  if (!normalized) {
    return rows;
  }
  return rows.filter((row) =>
    [row.name, row.type, row.preview].join("\n").toLowerCase().includes(normalized),
  );
}

export function stringifyJson(value: unknown) {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function valueType(value: unknown) {
  if (Array.isArray(value)) {
    return "array";
  }
  if (value === null) {
    return "null";
  }
  return typeof value;
}

function previewValue(value: unknown) {
  if (typeof value === "string") {
    return value;
  }
  if (typeof value === "number" || typeof value === "boolean" || value === null) {
    return String(value);
  }
  const json = stringifyJson(value).replace(/\s+/g, " ").trim();
  return json.length > 160 ? `${json.slice(0, 157)}...` : json;
}

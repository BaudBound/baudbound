import { listen } from "@tauri-apps/api/event";
import { Download } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";

import { CodeBlock } from "@/components/code-block";
import { EmptyState } from "@/components/empty-state";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { formatCount } from "@/lib/count-format";
import { SEARCH_INPUT_MAX_LENGTH } from "@/lib/input-limits";
import {
  exportVariables,
  getVariableInventory,
  type DeclaredVariableRecord,
  type StoredVariableRecord,
  type StoredVariableChange,
  type VariableInventory,
} from "@/lib/runner-api";
import { useDesktopTime } from "@/lib/time-format";

export function VariablesView({ scriptRevision }: { scriptRevision: string }) {
  const [inventory, setInventory] = useState<VariableInventory | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  const [search, setSearch] = useState("");
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<StoredVariableChange>(
      "runner-variable-changed",
      ({ payload }) => {
        setInventory((current) =>
          current ? applyVariableChange(current, payload) : current,
        );
      },
    ).then((cleanup) => {
      if (disposed) cleanup();
      else unlisten = cleanup;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
  useEffect(() => {
    let cancelled = false;
    void getVariableInventory()
      .then((result) => {
        if (!cancelled) {
          setInventory((current) => mergeInventory(result, current));
          setError(null);
        }
      })
      .catch((reason) => {
        if (!cancelled) setError(String(reason));
      });
    return () => {
      cancelled = true;
    };
  }, [scriptRevision]);

  if (error) return <EmptyState>Could not load variables: {error}</EmptyState>;
  if (!inventory) return <EmptyState>Loading variables...</EmptyState>;
  const query = search.trim().toLowerCase();
  const stored = inventory.stored.filter((item) =>
    matchesVariable(item, query),
  );
  const declared = inventory.declared.filter((item) =>
    matchesVariable(item, query),
  );

  async function exportInventory() {
    setExporting(true);
    try {
      const result = await exportVariables();
      if (!result.cancelled) {
        toast.success(
          `Exported ${formatCount(result.exported_count, "variable")} to ${result.file_name}.`,
        );
      }
    } catch (reason) {
      toast.error(`Could not export variables: ${String(reason)}`);
    } finally {
      setExporting(false);
    }
  }

  return (
    <div className="grid gap-4">
      <div className="flex min-w-0 flex-wrap items-center gap-2">
        <Input
          aria-label="Search variables"
          maxLength={SEARCH_INPUT_MAX_LENGTH}
          className="min-w-56 flex-1"
          onChange={(event) => setSearch(event.target.value)}
          placeholder="Search name, script, scope, type, description, or value"
          value={search}
        />
        <Button
          className="whitespace-nowrap"
          disabled={exporting}
          onClick={() => void exportInventory()}
          size="sm"
          variant="outline"
        >
          <Download />
          Export variables
        </Button>
      </div>
      {inventory.warnings.map((warning) => (
        <EmptyState key={warning}>{warning}</EmptyState>
      ))}
      <VariableSection title="Stored values">
        {stored.length === 0 ? (
          <EmptyState>No stored variables match the current search.</EmptyState>
        ) : (
          <StoredVariablesTable variables={stored} />
        )}
      </VariableSection>
      <VariableSection title="Declared defaults">
        {declared.length === 0 ? (
          <EmptyState>
            No declared defaults match the current search.
          </EmptyState>
        ) : (
          <DeclaredVariablesTable variables={declared} />
        )}
      </VariableSection>
    </div>
  );
}

function VariableSection({
  children,
  title,
}: {
  children: React.ReactNode;
  title: string;
}) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
      </CardHeader>
      <CardContent className="p-0 max-[1280px]:p-3">{children}</CardContent>
    </Card>
  );
}

function StoredVariablesTable({
  variables,
}: {
  variables: StoredVariableRecord[];
}) {
  const { formatUnixSeconds } = useDesktopTime();
  return (
    <table className="responsive-table w-full border-collapse text-sm">
      <thead>
        <tr className="border-b border-border text-left text-xs uppercase text-muted-foreground">
          <th className="px-3 py-2">Name</th>
          <th className="px-3 py-2">Scope</th>
          <th className="px-3 py-2">Script</th>
          <th className="px-3 py-2">Value</th>
          <th className="px-3 py-2">Updated</th>
        </tr>
      </thead>
      <tbody>
        {variables.map((variable) => (
          <tr
            className="border-b border-border align-top last:border-0"
            key={`${variable.scope}-${variable.script_id ?? "global"}-${variable.name}`}
          >
            <td className="px-3 py-3 font-mono text-xs" data-label="Name">
              {variable.name}
            </td>
            <td className="px-3 py-3" data-label="Scope">
              <Badge variant="muted">{variable.scope}</Badge>
            </td>
            <td className="px-3 py-3" data-label="Script">
              {variable.script_name ?? "All scripts"}
              {variable.script_id ? (
                <div className="break-all font-mono text-xs text-muted-foreground">
                  {variable.script_id}
                </div>
              ) : null}
            </td>
            <td className="max-w-[420px] px-3 py-3" data-label="Value">
              <CodeBlock className="max-h-40">
                {jsonValue(variable.value)}
              </CodeBlock>
            </td>
            <td className="whitespace-nowrap px-3 py-3" data-label="Updated">
              {formatUnixSeconds(variable.updated_at_unix)}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function DeclaredVariablesTable({
  variables,
}: {
  variables: DeclaredVariableRecord[];
}) {
  return (
    <table className="responsive-table w-full border-collapse text-sm">
      <thead>
        <tr className="border-b border-border text-left text-xs uppercase text-muted-foreground">
          <th className="px-3 py-2">Name</th>
          <th className="px-3 py-2">Scope</th>
          <th className="px-3 py-2">Type</th>
          <th className="px-3 py-2">Script</th>
          <th className="px-3 py-2">Default value</th>
          <th className="px-3 py-2">Description</th>
        </tr>
      </thead>
      <tbody>
        {variables.map((variable) => (
          <tr
            className="border-b border-border align-top last:border-0"
            key={`${variable.script_id}-${variable.name}`}
          >
            <td className="px-3 py-3 font-mono text-xs" data-label="Name">
              {variable.name}
            </td>
            <td className="px-3 py-3" data-label="Scope">
              <Badge variant="muted">{variable.scope}</Badge>
            </td>
            <td className="px-3 py-3" data-label="Type">
              <Badge variant="muted">{variable.value_type}</Badge>
            </td>
            <td className="px-3 py-3" data-label="Script">
              {variable.script_name}
              <div className="break-all font-mono text-xs text-muted-foreground">
                {variable.script_id}
              </div>
            </td>
            <td className="max-w-[420px] px-3 py-3" data-label="Default value">
              <CodeBlock className="max-h-40">
                {jsonValue(variable.value)}
              </CodeBlock>
            </td>
            <td
              className="px-3 py-3 text-muted-foreground"
              data-label="Description"
            >
              {variable.description || "No description"}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function jsonValue(value: unknown) {
  return JSON.stringify(value, null, 2) ?? String(value);
}

function matchesVariable(
  variable: StoredVariableRecord | DeclaredVariableRecord,
  query: string,
) {
  if (!query) return true;
  return [
    variable.name,
    variable.scope,
    variable.script_name ?? "",
    "value_type" in variable ? variable.value_type : "",
    "description" in variable ? variable.description : "",
    jsonValue(variable.value),
  ]
    .join("\n")
    .toLowerCase()
    .includes(query);
}

function applyVariableChange(
  inventory: VariableInventory,
  change: StoredVariableChange,
): VariableInventory {
  const keyMatches = (stored: StoredVariableRecord) =>
    stored.scope === change.scope &&
    stored.name === change.name &&
    stored.script_id === change.script_id;
  const existing = inventory.stored.find(keyMatches);
  const scriptName =
    existing?.script_name ??
    (change.script_id ? inventory.script_names[change.script_id] : undefined) ??
    null;
  const next: StoredVariableRecord = { ...change, script_name: scriptName };
  return {
    ...inventory,
    stored: sortStoredVariables(
      existing
        ? inventory.stored.map((item) => (keyMatches(item) ? next : item))
        : [...inventory.stored, next],
    ),
  };
}

function mergeInventory(
  loaded: VariableInventory,
  current: VariableInventory | null,
): VariableInventory {
  if (!current) {
    return { ...loaded, stored: sortStoredVariables(loaded.stored) };
  }
  const merged = new Map<string, StoredVariableRecord>();
  for (const variable of loaded.stored) {
    merged.set(variableKey(variable), variable);
  }
  for (const variable of current.stored) {
    const key = variableKey(variable);
    const loadedVariable = merged.get(key);
    if (!loadedVariable || variable.version > loadedVariable.version) {
      merged.set(key, variable);
    }
  }
  return {
    ...loaded,
    stored: sortStoredVariables([...merged.values()]),
  };
}

function variableKey(variable: StoredVariableRecord) {
  return `${variable.scope}\u0000${variable.script_id ?? ""}\u0000${variable.name}`;
}

function sortStoredVariables(variables: StoredVariableRecord[]) {
  return [...variables].sort((left, right) => {
    const scriptOrder = (left.script_name ?? "").localeCompare(
      right.script_name ?? "",
    );
    return scriptOrder || left.name.localeCompare(right.name);
  });
}

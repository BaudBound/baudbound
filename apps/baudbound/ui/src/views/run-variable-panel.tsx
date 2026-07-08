import { ChevronDown, ChevronUp } from "lucide-react";
import { useMemo, useState } from "react";

import { CodeBlock } from "@/components/code-block";
import { EmptyState } from "@/components/empty-state";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  filterVariables,
  stringifyJson,
  variableRows,
  type VariableRow,
} from "@/lib/run-inspection";

export function RunVariablePanel({
  variables,
}: {
  variables: Record<string, unknown>;
}) {
  const [query, setQuery] = useState("");
  const [expandedNames, setExpandedNames] = useState<Set<string>>(new Set());
  const rows = useMemo(() => variableRows(variables), [variables]);
  const visibleRows = useMemo(() => filterVariables(rows, query), [query, rows]);

  function toggleExpanded(name: string) {
    setExpandedNames((current) => {
      const next = new Set(current);
      if (next.has(name)) {
        next.delete(name);
      } else {
        next.add(name);
      }
      return next;
    });
  }

  if (rows.length === 0) {
    return <EmptyState>No variables were captured for this run.</EmptyState>;
  }

  return (
    <div className="grid gap-3">
      <Input
        aria-label="Search run variables"
        onChange={(event) => setQuery(event.target.value)}
        placeholder="Search variable name, type, or value"
        value={query}
      />

      {visibleRows.length === 0 ? (
        <EmptyState>No variables match the current search.</EmptyState>
      ) : (
        <div className="max-h-[420px] overflow-auto rounded-md border border-border p-0 max-[900px]:border-0">
          <table className="responsive-table w-full border-collapse text-sm">
            <thead>
              <tr className="border-b border-border text-left text-xs uppercase text-muted-foreground">
                <th className="px-3 py-2">Name</th>
                <th className="px-3 py-2">Type</th>
                <th className="px-3 py-2">Value</th>
                <th className="px-3 py-2"></th>
              </tr>
            </thead>
            <tbody>
              {visibleRows.map((row) => (
                <VariableTableRow
                  expanded={expandedNames.has(row.name)}
                  key={row.name}
                  onToggle={() => toggleExpanded(row.name)}
                  row={row}
                />
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

function VariableTableRow({
  expanded,
  onToggle,
  row,
}: {
  expanded: boolean;
  onToggle: () => void;
  row: VariableRow;
}) {
  const expandable = row.type === "object" || row.type === "array";

  return (
    <>
      <tr className="border-b border-border align-top last:border-b-0">
        <td className="px-3 py-2 font-mono text-xs" data-label="Name">
          {row.name}
        </td>
        <td className="px-3 py-2" data-label="Type">
          <Badge variant="muted">{row.type}</Badge>
        </td>
        <td className="max-w-[420px] px-3 py-2" data-label="Value">
          <span className="break-words font-mono text-xs text-muted-foreground">
            {row.preview}
          </span>
        </td>
        <td className="px-3 py-2 text-right" data-label="Actions">
          {expandable ? (
            <Button
              aria-label={`${expanded ? "Hide" : "Show"} ${row.name} JSON`}
              className="size-7 p-0"
              onClick={onToggle}
              size="sm"
              title={expanded ? "Hide JSON" : "Show JSON"}
              variant={expanded ? "secondary" : "outline"}
            >
              {expanded ? <ChevronUp /> : <ChevronDown />}
            </Button>
          ) : null}
        </td>
      </tr>
      {expanded ? (
        <tr className="border-b border-border bg-background/40">
          <td className="p-3" colSpan={4} data-label="">
            <CodeBlock className="max-h-[280px]">{stringifyJson(row.raw)}</CodeBlock>
          </td>
        </tr>
      ) : null}
    </>
  );
}

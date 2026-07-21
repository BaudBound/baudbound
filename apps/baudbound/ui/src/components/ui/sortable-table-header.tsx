import { ArrowDown, ArrowUp } from "lucide-react";
import type { ReactNode } from "react";

import type { SortDirection, SortState } from "@/lib/table-sorting";
import { cn } from "@/lib/utils";

export function SortableTableHeader<Column extends string>({
  children,
  className,
  column,
  onSort,
  sortState,
}: {
  children: ReactNode;
  className?: string;
  column: Column;
  onSort: (column: Column) => void;
  sortState: SortState<Column>;
}) {
  const direction: SortDirection | null =
    sortState?.column === column ? sortState.direction : null;

  return (
    <th
      aria-sort={direction ?? "none"}
      className={cn("px-3 py-2", className)}
    >
      <button
        className="-mx-1 inline-flex max-w-full items-center gap-1 rounded-sm px-1 py-0.5 text-left hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        onClick={() => onSort(column)}
        type="button"
      >
        <span>{children}</span>
        <span aria-hidden="true" className="size-3.5 shrink-0">
          {direction === "ascending" ? <ArrowUp className="size-3.5" /> : null}
          {direction === "descending" ? <ArrowDown className="size-3.5" /> : null}
        </span>
      </button>
    </th>
  );
}

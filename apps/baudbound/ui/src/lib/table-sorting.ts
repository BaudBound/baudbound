import { useCallback, useMemo, useState } from "react";

export type SortDirection = "ascending" | "descending";
export type SortState<Column extends string> = {
  column: Column;
  direction: SortDirection;
} | null;

export type SortValue = boolean | number | string | null | undefined;

const textCollator = new Intl.Collator(undefined, {
  numeric: true,
  sensitivity: "base",
});

export function nextSortState<Column extends string>(
  current: SortState<Column>,
  column: Column,
): SortState<Column> {
  if (!current || current.column !== column) {
    return { column, direction: "ascending" };
  }
  if (current.direction === "ascending") {
    return { column, direction: "descending" };
  }
  return null;
}

export function sortRows<Row, Column extends string>(
  rows: readonly Row[],
  sortState: SortState<Column>,
  selectors: Record<Column, (row: Row) => SortValue>,
): Row[] {
  if (!sortState) return [...rows];

  const selector = selectors[sortState.column];
  const direction = sortState.direction === "ascending" ? 1 : -1;
  return rows
    .map((row, index) => ({ index, row }))
    .sort((left, right) => {
      const leftValue = selector(left.row);
      const rightValue = selector(right.row);
      if (leftValue == null && rightValue == null) return left.index - right.index;
      if (leftValue == null) return 1;
      if (rightValue == null) return -1;

      const compared = compareSortValues(leftValue, rightValue);
      return compared === 0 ? left.index - right.index : compared * direction;
    })
    .map(({ row }) => row);
}

export function useSortableRows<Row, Column extends string>(
  rows: readonly Row[],
  selectors: Record<Column, (row: Row) => SortValue>,
) {
  const [sortState, setSortState] = useState<SortState<Column>>(null);
  const toggleSort = useCallback((column: Column) => {
    setSortState((current) => nextSortState(current, column));
  }, []);
  const sortedRows = useMemo(
    () => sortRows(rows, sortState, selectors),
    [rows, selectors, sortState],
  );

  return { sortedRows, sortState, toggleSort };
}

function compareSortValues(left: SortValue, right: SortValue) {
  if (typeof left === "number" && typeof right === "number") return left - right;
  if (typeof left === "boolean" && typeof right === "boolean") {
    return Number(left) - Number(right);
  }
  return textCollator.compare(String(left), String(right));
}

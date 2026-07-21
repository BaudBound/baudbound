/// <reference types="vite/client" />

import { describe, expect, it } from "vitest";

import { nextSortState, sortRows } from "@/lib/table-sorting";

describe("table sorting", () => {
  it("cycles a column through ascending, descending, and natural order", () => {
    const ascending = nextSortState(null, "name");
    const descending = nextSortState(ascending, "name");

    expect(ascending).toEqual({ column: "name", direction: "ascending" });
    expect(descending).toEqual({ column: "name", direction: "descending" });
    expect(nextSortState(descending, "name")).toBeNull();
    expect(nextSortState(descending, "status")).toEqual({
      column: "status",
      direction: "ascending",
    });
  });

  it("sorts text naturally and preserves equal row order", () => {
    const rows = [
      { id: 1, name: "Script 10" },
      { id: 2, name: "script 2" },
      { id: 3, name: "SCRIPT 2" },
    ];
    const selectors = { name: (row: (typeof rows)[number]) => row.name };

    expect(
      sortRows(rows, { column: "name", direction: "ascending" }, selectors).map(
        (row) => row.id,
      ),
    ).toEqual([2, 3, 1]);
    expect(sortRows(rows, null, selectors)).toEqual(rows);
  });

  it("keeps missing values last in both directions", () => {
    const rows = [{ value: 2 }, { value: null }, { value: 1 }];
    const selectors = { value: (row: (typeof rows)[number]) => row.value };

    expect(
      sortRows(rows, { column: "value", direction: "ascending" }, selectors).map(
        (row) => row.value,
      ),
    ).toEqual([1, 2, null]);
    expect(
      sortRows(rows, { column: "value", direction: "descending" }, selectors).map(
        (row) => row.value,
      ),
    ).toEqual([2, 1, null]);
  });
});

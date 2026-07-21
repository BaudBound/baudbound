/// <reference types="vite/client" />

import { describe, expect, it } from "vitest";

import {
  filterVariableMetadata,
  variableScopeLabel,
  variableRows,
} from "@/lib/run-inspection";

describe("run variable inspection", () => {
  it("hides generated metadata variables by default", () => {
    const rows = variableRows({
      value: "hello",
      "value.$count": 5,
      "value.$is_empty": false,
      "value.$length": 5,
      "value.$type": "string",
    });

    expect(filterVariableMetadata(rows, false).map((row) => row.name)).toEqual([
      "value",
    ]);
    expect(filterVariableMetadata(rows, true)).toEqual(rows);
  });

  it("does not hide ordinary names that only contain metadata words", () => {
    const rows = variableRows({
      "value.$length.extra": 1,
      value_type: "text",
    });

    expect(filterVariableMetadata(rows, false)).toEqual(rows);
  });

  it("uses recorded scopes and identifies metadata in older records", () => {
    const rows = variableRows(
      {
        counter: 3,
        "counter.$type": "number",
        "n-action.result": "done",
      },
      {
        counter: "persistent",
        "n-action.result": "node_output",
      },
    );

    expect(rows.map(({ name, scope }) => ({ name, scope }))).toEqual([
      { name: "counter", scope: "persistent" },
      { name: "counter.$type", scope: "metadata" },
      { name: "n-action.result", scope: "node_output" },
    ]);
    expect(variableScopeLabel(rows[2].scope)).toBe("Node output");
  });
});

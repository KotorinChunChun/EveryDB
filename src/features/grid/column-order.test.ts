import { describe, expect, it } from "vitest";
import {
  applyColumnAliasChange,
  columnMapVisibleDefault,
  reorderDisplayColumns,
  sourceColumnOrderForDisplay,
} from "./column-order";
import { PINNED_GRID_COLUMNS } from "./data";

describe("reorderDisplayColumns", () => {
  const cols = ["#", "テーブル", "元行", "見出し", "A", "B", "C"];

  it("moves a column before the target", () => {
    expect(reorderDisplayColumns(cols, PINNED_GRID_COLUMNS, "C", "A", false)).toEqual(["C", "A", "B"]);
  });

  it("moves a column after the target", () => {
    expect(reorderDisplayColumns(cols, PINNED_GRID_COLUMNS, "A", "B", true)).toEqual(["B", "A", "C"]);
  });

  it("returns null for invalid drag", () => {
    expect(reorderDisplayColumns(cols, PINNED_GRID_COLUMNS, "A", "#", false)).toBeNull();
  });
});

describe("sourceColumnOrderForDisplay", () => {
  it("sorts raw fields by display order with aliases", () => {
    const order = sourceColumnOrderForDisplay(
      ["f1", "f2", "f3"],
      { f1: "Alpha", f3: "Gamma" },
      ["Alpha", "Beta", "Gamma"],
    );
    expect(order).toEqual(["f1", "f3", "f2"]);
  });
});

describe("applyColumnAliasChange", () => {
  it("removes old alias and sets new alias", () => {
    expect(
      applyColumnAliasChange({ old: "Name" }, "old", "Name", "new", "Alias"),
    ).toEqual({ new: "Alias" });
  });
});

describe("columnMapVisibleDefault", () => {
  it("returns false when any mapped field is hidden", () => {
    expect(
      columnMapVisibleDefault([{ currentField: "f", displayColumns: [] }]),
    ).toBe(false);
  });

  it("returns null when nothing is mapped", () => {
    expect(columnMapVisibleDefault([{ currentField: null }])).toBeNull();
  });
});

import { describe, expect, it } from "vitest";
import type { Row } from "../../shared/types";
import {
  buildGridColumns,
  buildSourceFieldIndex,
  gridCellText,
  gridColumnWidth,
  nextSortState,
  sortedGridRows,
  resizedGridColumnWidth,
  toRawFieldNames,
} from "./data";

function row(id: number, name: string, fields: Record<string, string>, label: string | null = null): Row {
  return { source_id: id, source_name: name, label, open_ref: null, row_number: id, fields };
}

describe("grid data helpers", () => {
  it("uses system column defaults and clamps resized widths", () => {
    expect(gridColumnWidth("#", {}, 200)).toBe(50);
    expect(gridColumnWidth("元行", {}, 200)).toBe(55);
    expect(gridColumnWidth("氏名", { 氏名: 280 }, 200)).toBe(280);
    expect(resizedGridColumnWidth(80, -100)).toBe(40);
    expect(resizedGridColumnWidth(80, 20)).toBe(100);
  });
  it("keeps configured field order and appends newly seen fields", () => {
    const rows = [row(1, "A", { name: "one", age: "10" }), row(2, "B", { city: "Tokyo", name: "two" })];
    expect(buildGridColumns(rows, ["city", "missing", "name"])).toEqual([
      "#", "テーブル", "元行", "見出し", "city", "name", "age",
    ]);
    expect(gridCellText(rows[0], "#", 0)).toBe("1");
    expect(gridCellText(rows[0], "テーブル")).toBe("A");
    expect(gridCellText(rows[0], "age")).toBe("10");
  });

  it("sorts numeric values naturally, keeps empty values last, and preserves ties", () => {
    const rows = [row(1, "A", { value: "10" }), row(2, "B", { value: "2" }), row(3, "C", { value: "" }), row(4, "D", { value: "2" })];
    const result = sortedGridRows(rows, ["value"], { column: "value", dir: "asc" });
    expect(result.rows.map((item) => item.source_id)).toEqual([2, 4, 1, 3]);
    expect(sortedGridRows(rows, ["other"], { column: "value", dir: "asc" }).sortState).toBeNull();
  });

  it("cycles sort state and converts display aliases back to raw fields", () => {
    expect(nextSortState(null, "name")).toEqual({ column: "name", dir: "asc" });
    expect(nextSortState({ column: "name", dir: "asc" }, "name")).toEqual({ column: "name", dir: "desc" });
    expect(nextSortState({ column: "name", dir: "desc" }, "name")).toBeNull();
    expect(toRawFieldNames(["顧客名", "age", "顧客名"], { customer_name: "顧客名" })).toEqual(["customer_name", "age"]);
    expect([...buildSourceFieldIndex([row(1, "A", { x: "1" }), row(1, "A", { y: "2" })]).get(1)!]).toEqual(["x", "y"]);
  });

  it("RF-50-08: column build and sort stay fast for 10k rows", () => {
    const rows: Row[] = Array.from({ length: 10_000 }, (_, index) =>
      row(index + 1, "Bench", { value: String(index).padStart(5, "0"), note: `row-${index}` }, `Row ${index}`),
    );
    const started = performance.now();
    const columns = buildGridColumns(rows, ["value", "note"]);
    const { rows: sorted } = sortedGridRows(rows, columns, { column: "value", dir: "asc" });
    const elapsed = performance.now() - started;
    expect(sorted.length).toBe(10_000);
    expect(columns).toContain("value");
    expect(elapsed).toBeLessThan(2000);
  });
});
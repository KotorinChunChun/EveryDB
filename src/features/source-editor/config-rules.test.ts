import { describe, expect, it } from "vitest";
import type { FieldDef } from "../../source-types/forms";
import { isConfigIncomplete, withFieldVisibility } from "./config-rules";

const fields: FieldDef[] = [
  { key: "path", label: "ファイル", kind: "text" },
  { key: "token", label: "トークン", kind: "password", optional: true },
];

describe("source editor config rules", () => {
  it("requires text fields and validates manually selected Excel cells", () => {
    expect(isConfigIncomplete("csv", fields, {})).toBe(true);
    expect(isConfigIncomplete("csv", fields, { path: "C:/data.csv" })).toBe(false);
    expect(isConfigIncomplete("excel_range", fields, { path: "C:/book.xlsx", start_mode: "manual", start_cell: "A0" })).toBe(true);
    expect(isConfigIncomplete("excel_range", fields, { path: "C:/book.xlsx", start_mode: "manual", start_cell: "A1", end_mode: "manual", end_cell: "XFD1048576" })).toBe(false);
  });

  it("preserves all-visible semantics while toggling a mapped field", () => {
    expect(withFieldVisibility(undefined, ["id", "name"], "name", true)).toBeUndefined();
    expect(withFieldVisibility(undefined, ["id", "name"], "name", false)).toEqual(["id"]);
    expect(withFieldVisibility(["id"], ["id", "name"], "name", true)).toEqual(["id", "name"]);
    expect(withFieldVisibility(["id", "name"], ["id", "name"], "name", false)).toEqual(["id"]);
  });
});
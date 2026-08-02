import { describe, expect, it } from "vitest";
import { basenameOf, deriveDefaultName } from "./default-name";

describe("source editor default name", () => {
  it("normalizes quoted Windows paths and derives file-based names", () => {
    expect(basenameOf('  "C:\\Data\\customers.csv"  ')).toBe("customers.csv");
    expect(deriveDefaultName("excel_table", { path: "C:/Data/book.xlsx", table: "売上" })).toBe("book.xlsx: 売上");
    expect(deriveDefaultName("excel_range", { path: "C:/Data/book.xlsx", sheet: "Sheet1", start_cell: "A1", end_cell: "D10" })).toBe("book.xlsx: Sheet1!A1:D10");
  });

  it("prefers display captions and preserves safe fallbacks", () => {
    expect(deriveDefaultName("airtable", { base_name: "営業", table_name: "案件", view_name: "進行中" })).toBe("営業: 案件 (進行中)");
    expect(deriveDefaultName("web_page", { url: "https://example.com/docs/guide" })).toBe("example.com/guide");
    expect(deriveDefaultName("graph_sharepoint_library", {})).toBe("ドキュメントライブラリ");
  });
});
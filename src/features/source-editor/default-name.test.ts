import { describe, expect, it } from "vitest";
import { basenameOf, deriveDefaultName, shouldUpdateNotionDisplayName } from "./default-name";

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
    expect(deriveDefaultName("graph_sharepoint_list", { site_display_name: "営業サイト" })).toBe("営業サイト");
    expect(deriveDefaultName("graph_sharepoint_list", { site_display_name: "営業サイト", list_name: "案件" })).toBe("案件");
    expect(deriveDefaultName("notion", { database_id: "0123456789abcdef", database_name: "案件管理" })).toBe("案件管理");
  });

  it("updates only automatic Notion names and preserves a user label", () => {
    expect(shouldUpdateNotionDisplayName("旧DB名", "旧DB名", "0123456789abcdef")).toBe(true);
    expect(shouldUpdateNotionDisplayName("Notion DB (01234567...)", "", "0123456789abcdef")).toBe(true);
    expect(shouldUpdateNotionDisplayName("経理用の独自名", "旧DB名", "0123456789abcdef")).toBe(false);
  });
});

import { beforeEach, describe, expect, it } from "vitest";
import { colLetterOf, colLetterToIndex, formatSelectedColumns, parseColumnSelection } from "./excel-columns";
import { loadNumberArray, loadString, loadStringArray, saveJson, saveString } from "./storage";
import { getCellUrl, parseAirtableUrl, parseGoogleSheetsUrl, parseNotionUrl, parseSharePointUrl } from "./urls";

describe("URL helpers", () => {
  it("recognizes supported opener schemes and www", () => {
    expect(getCellUrl(" https://example.com ")).toBe("https://example.com");
    expect(getCellUrl("www.example.com")).toBe("https://www.example.com");
    expect(getCellUrl("notion://www.notion.so/page")).toBe("notion://www.notion.so/page");
    expect(getCellUrl("javascript:alert(1)")).toBeNull();
  });

  it("extracts IDs from service URLs", () => {
    expect(parseAirtableUrl("https://airtable.com/app12345678901/tbl12345678901/viw12345678901")).toEqual({ baseId: "app12345678901", tableId: "tbl12345678901", viewId: "viw12345678901" });
    expect(parseGoogleSheetsUrl("https://docs.google.com/spreadsheets/d/sheet_123/edit")).toEqual({ spreadsheetId: "sheet_123" });
    expect(parseNotionUrl("https://www.notion.so/01234567-89ab-cdef-0123-456789abcdef")).toEqual({ databaseId: "0123456789abcdef0123456789abcdef" });
    expect(parseSharePointUrl("https://tenant.sharepoint.com/sites/team/Lists/My%20List/AllItems.aspx")).toEqual({ siteUrl: "https://tenant.sharepoint.com/sites/team", listName: "My List", libraryName: undefined });
  });
});

describe("Excel column helpers", () => {
  it("round-trips Excel column letters", () => {
    expect(colLetterOf(0)).toBe("A");
    expect(colLetterOf(26)).toBe("AA");
    expect(colLetterToIndex("XFD")).toBe(16383);
    expect(colLetterToIndex("A1")).toBe(-1);
  });

  it("parses and formats selected column ranges", () => {
    expect([...parseColumnSelection("A:C, E; G:F")]).toEqual([0, 1, 2, 4, 5, 6]);
    expect(formatSelectedColumns(new Set([0, 1, 2, 4, 5, 7]))).toBe("A:C, E:F, H");
  });
});

describe("storage helpers", () => {
  beforeEach(() => localStorage.clear());

  it("persists strings and validates JSON array item types", () => {
    saveString("name", "EveryDB");
    saveJson("numbers", [1, 2]);
    saveJson("mixed", [1, "2"]);
    saveJson("strings", ["a", "b"]);
    expect(loadString("name")).toBe("EveryDB");
    expect(loadNumberArray("numbers")).toEqual([1, 2]);
    expect(loadNumberArray("mixed")).toBeNull();
    expect(loadStringArray("strings")).toEqual(["a", "b"]);
  });
});

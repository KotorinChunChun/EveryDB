import { describe, expect, it } from "vitest";
import {
  isNotionNameResolved,
  isApiOwnedSourceName,
  normalizeNotionDatabaseId,
  notionNameFailureMessage,
} from "./notion-name-policy";

describe("Notion name policy", () => {
  it("compares database IDs independent of hyphen and letter case", () => {
    expect(normalizeNotionDatabaseId("01234567-89AB-CDEF-0123-456789ABCDEF")).toBe(
      "0123456789abcdef0123456789abcdef",
    );
    expect(isNotionNameResolved(
      "01234567-89ab-cdef-0123-456789abcdef",
      "0123456789ABCDEF0123456789ABCDEF",
      "案件管理",
    )).toBe(true);
  });

  it("marks only Notion source names as API-owned", () => {
    expect(isApiOwnedSourceName("notion")).toBe(true);
    expect(isApiOwnedSourceName("csv")).toBe(false);
  });

  it("rejects a stale name that belongs to a different database ID", () => {
    expect(isNotionNameResolved(
      "1123456789abcdef0123456789abcdef",
      "0123456789abcdef0123456789abcdef",
      "以前のデータベース名",
    )).toBe(false);
    expect(isNotionNameResolved("0123456789abcdef0123456789abcdef", "", "案件管理")).toBe(false);
  });

  it("explains whether a saved name was preserved after failure", () => {
    expect(notionNameFailureMessage("offline", true)).toContain("維持しています");
    expect(notionNameFailureMessage("offline", false)).toContain("再試行してください");
  });
});

import { describe, expect, it } from "vitest";
import type { SourceInfo } from "../../shared/types";
import { parentLabel, shortTimestamp, sourceMatchesFilter, sourceTree, sourceTypeLabel } from "./tree";

function source(id: number, name: string, parent = ""): SourceInfo {
  return {
    id, type_: "csv", name, parent, refresh_interval: 60, status: null,
    last_success_at: null, last_error: null, record_count: 0,
  };
}

describe("source tree helpers", () => {
  it("keeps source order while grouping children by their parent", () => {
    const entries = sourceTree([
      source(1, "single"), source(2, "first", "C:\\data\\book.xlsx"),
      source(3, "second", "C:\\data\\book.xlsx"), source(4, "last"),
    ]);

    expect(entries.map((entry) => entry.kind === "source" ? entry.source.id : entry.children.map((child) => child.id))).toEqual([
      1, [2, 3], 4,
    ]);
  });

  it("formats labels, timestamps, and filters by source or parent", () => {
    const overrides = new Map([["service", "営業データ"]]);
    expect(parentLabel("C:\\data\\book.xlsx", overrides)).toBe("book.xlsx");
    expect(parentLabel("service", overrides)).toBe("営業データ");
    expect(sourceTypeLabel("csv")).toBe("CSV ファイル");
    expect(shortTimestamp("2026-08-02 13:45:00")).toBe("08-02 13:45");
    expect(sourceMatchesFilter(source(1, "customers", "service"), "営業", overrides)).toBe(true);
    expect(sourceMatchesFilter(source(1, "customers", "service"), "missing", overrides)).toBe(false);
  });
});
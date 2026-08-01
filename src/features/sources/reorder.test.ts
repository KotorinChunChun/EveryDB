import { describe, expect, it } from "vitest";
import type { SourceInfo } from "../../shared/types";
import { reorderedSourceIds } from "./reorder";

function source(id: number, parent = ""): SourceInfo {
  return {
    id, type_: "csv", name: String(id), parent, refresh_interval: 60, status: null,
    last_success_at: null, last_error: null, record_count: 0,
  };
}

const entries = [source(1), source(2, "a"), source(3, "a"), source(4), source(5, "b"), source(6, "b")];

describe("reorderedSourceIds", () => {
  it("moves one source before or after another source", () => {
    expect(reorderedSourceIds(entries, 4, null, null, 2, false)).toEqual([1, 4, 2, 3, 5, 6]);
    expect(reorderedSourceIds(entries, 1, null, null, 3, true)).toEqual([2, 3, 1, 4, 5, 6]);
  });

  it("moves an entire parent group around another group", () => {
    expect(reorderedSourceIds(entries, null, "b", "a", null, false)).toEqual([1, 5, 6, 2, 3, 4]);
    expect(reorderedSourceIds(entries, null, "a", "b", null, true)).toEqual([1, 4, 5, 6, 2, 3]);
  });

  it("returns null for no drag or an unchanged position", () => {
    expect(reorderedSourceIds(entries, null, null, null, 1, false)).toBeNull();
    expect(reorderedSourceIds(entries, 2, null, null, 3, false)).toBeNull();
    expect(reorderedSourceIds(entries, null, "a", "a", null, false)).toBeNull();
  });
});
import { describe, expect, it } from "vitest";
import type { SourceInfo } from "../../shared/types";
import { collapseAllParents, expandAllParents, showChildrenWhileFiltered, toggleParentCollapsed } from "./collapse";

function source(id: number, parent: string): SourceInfo {
  return {
    id,
    type_: "csv",
    name: `s${id}`,
    parent,
    record_count: 0,
    refresh_interval: 3600,
    status: "ok",
    last_success_at: null,
    last_error: null,
  };
}

describe("source collapse helpers", () => {
  it("expands and collapses all parent groups", () => {
    const collapsed = new Set(["a", "b"]);
    expandAllParents(collapsed);
    expect(collapsed.size).toBe(0);
    collapseAllParents(collapsed, [source(1, "p1"), source(2, "p1"), source(3, "")]);
    expect([...collapsed]).toEqual(["p1"]);
  });

  it("toggles one parent and respects filter visibility", () => {
    const collapsed = new Set<string>();
    expect(toggleParentCollapsed(collapsed, "p1")).toBe(true);
    expect(collapsed.has("p1")).toBe(true);
    expect(toggleParentCollapsed(collapsed, "p1")).toBe(false);
    expect(showChildrenWhileFiltered(true, true)).toBe(true);
    expect(showChildrenWhileFiltered(true, false)).toBe(false);
  });
});

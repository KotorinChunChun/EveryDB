import { describe, expect, it } from "vitest";
import {
  includeNewSourceIds,
  restoreSelectedIds,
  selectAllIfEmpty,
  setVisibleSelection,
  toggleParentSelection,
  toggleSourceSelection,
} from "./selection";

describe("source selection helpers", () => {
  it("restores only valid IDs and selects all only when no selection exists", () => {
    expect([...restoreSelectedIds([1, 3, 9], [1, 2, 3])]).toEqual([1, 3]);
    expect([...selectAllIfEmpty(new Set(), [1, 2])]).toEqual([1, 2]);
    expect([...selectAllIfEmpty(new Set([2]), [1, 2])]).toEqual([2]);
  });

  it("keeps existing selections while including newly added sources", () => {
    expect([...includeNewSourceIds(new Set([1]), [1, 2], [1, 2, 3])]).toEqual([1, 3]);
  });

  it("updates single, parent, and visible selections including Alt-only behavior", () => {
    expect([...toggleSourceSelection(new Set([1, 2]), 3, true, false)]).toEqual([1, 2, 3]);
    expect([...toggleSourceSelection(new Set([1, 2]), 3, true, true)]).toEqual([3]);
    expect([...toggleParentSelection(new Set([1, 2, 4]), [1, 2], false)]).toEqual([4]);
    expect([...toggleParentSelection(new Set([4]), [1, 2], false)]).toEqual([4, 1, 2]);
    expect([...toggleParentSelection(new Set([4]), [1, 2], true)]).toEqual([1, 2]);
    expect([...setVisibleSelection(new Set([1, 4]), [2, 3], false)]).toEqual([1, 4]);
  });
});
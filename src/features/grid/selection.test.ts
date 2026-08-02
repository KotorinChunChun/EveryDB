import { describe, expect, it } from "vitest";
import { cellSelectionSuffix, initialCellPosition, isArrowKey, moveCellSelection } from "./selection";

describe("grid selection helpers", () => {
  it("moves within bounds and marks selected cell", () => {
    expect(moveCellSelection({ row: 1, col: 1 }, "ArrowDown", 3, 4)).toEqual({ row: 2, col: 1 });
    expect(moveCellSelection({ row: 0, col: 0 }, "ArrowUp", 3, 4)).toEqual({ row: 0, col: 0 });
    expect(cellSelectionSuffix({ row: 2, col: 1 }, 2, 1)).toBe(" sel");
    expect(cellSelectionSuffix({ row: 2, col: 1 }, 1, 1)).toBe("");
  });

  it("recognizes arrow keys and default cell", () => {
    expect(isArrowKey("ArrowLeft")).toBe(true);
    expect(isArrowKey("Enter")).toBe(false);
    expect(initialCellPosition()).toEqual({ row: 0, col: 0 });
  });
});

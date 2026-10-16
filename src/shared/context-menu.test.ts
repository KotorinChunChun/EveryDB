import { describe, expect, it } from "vitest";
import { clampContextMenuPosition } from "./context-menu";

describe("context menu positioning", () => {
  it("keeps the menu inside the viewport with a small margin", () => {
    expect(clampContextMenuPosition(10, 20, 120, 80, 800, 600)).toEqual({ left: 10, top: 20 });
    expect(clampContextMenuPosition(790, 590, 120, 80, 800, 600)).toEqual({ left: 676, top: 516 });
    expect(clampContextMenuPosition(0, 0, 200, 100, 800, 600)).toEqual({ left: 4, top: 4 });
  });
});

import { describe, expect, it } from "vitest";
import { scrollTopToRevealRow, visibleRowRange } from "./viewport";

describe("grid viewport helpers", () => {
  it("adds overscan while clamping the visible row range", () => {
    expect(visibleRowRange(100, 260, 100, 26)).toEqual({ start: 5, end: 19 });
    expect(visibleRowRange(3, -1, 100, 26)).toEqual({ start: 0, end: 3 });
    expect(visibleRowRange(0, 0, 100, 26)).toEqual({ start: 0, end: 0 });
  });

  it("scrolls only when a requested row is outside the viewport", () => {
    expect(scrollTopToRevealRow(2, 26, 100, 26)).toBe(26);
    expect(scrollTopToRevealRow(0, 52, 100, 26)).toBe(0);
    expect(scrollTopToRevealRow(8, 52, 100, 26)).toBe(134);
  });
});
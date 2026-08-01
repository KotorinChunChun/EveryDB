import { describe, expect, it } from "vitest";
import { normalizeRowLimit, presetSourceIds, shouldApplySearchResponse } from "./state";

describe("search state helpers", () => {
  it("normalizes invalid row limits to the configured fallback", () => {
    expect(normalizeRowLimit("250")).toBe(250);
    expect(normalizeRowLimit("0")).toBe(10_000);
    expect(normalizeRowLimit("abc", 50)).toBe(50);
  });

  it("applies only responses newer than the displayed sequence", () => {
    expect(shouldApplySearchResponse(4, 3)).toBe(true);
    expect(shouldApplySearchResponse(3, 3)).toBe(false);
    expect(shouldApplySearchResponse(2, 3)).toBe(false);
  });

  it("removes deleted sources when applying a preset", () => {
    expect([...presetSourceIds([1, 3, 9], [1, 2, 3])]).toEqual([1, 3]);
  });
});
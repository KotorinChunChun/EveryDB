import { describe, expect, it } from "vitest";
import { manageModeSidebarWidth } from "./manage-mode";

describe("manage mode sidebar width", () => {
  it("clamps between minimum and available window width", () => {
    expect(manageModeSidebarWidth(500, 1200)).toBe(720);
    expect(manageModeSidebarWidth(900, 1200)).toBe(880);
    expect(manageModeSidebarWidth(2000, 1200)).toBe(880);
  });
});

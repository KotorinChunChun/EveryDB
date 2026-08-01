import { describe, expect, it, vi } from "vitest";
import { renderHistoryPanel, renderPresetPanel } from "./panels";

describe("search panel renderers", () => {
  it("renders an empty history message and invokes the selected history callback", () => {
    const panel = document.createElement("div");
    const select = vi.fn();
    renderHistoryPanel(panel, [], select);
    expect(panel.textContent).toBe("検索履歴はありません");

    renderHistoryPanel(panel, ["<tag>", "second"], select);
    expect(panel.children).toHaveLength(2);
    expect(panel.firstElementChild?.textContent).toBe("<tag>");
    (panel.firstElementChild as HTMLElement).dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    expect(select).toHaveBeenCalledWith("<tag>");
  });

  it("renders presets and keeps delete separate from selection", () => {
    const panel = document.createElement("div");
    const preset = { id: 1, name: "重要", query: "alpha", source_ids: [2], created_at: "2026-01-01" };
    const onSelect = vi.fn();
    const onDelete = vi.fn();
    renderPresetPanel(panel, [preset], { onSelect, onDelete });

    expect(panel.textContent).toContain("重要（alpha）");
    const row = panel.firstElementChild as HTMLElement;
    (row.querySelector(".preset-label") as HTMLElement).dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    (row.querySelector(".preset-del") as HTMLElement).dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    expect(onSelect).toHaveBeenCalledWith(preset);
    expect(onDelete).toHaveBeenCalledWith(preset);
  });
});
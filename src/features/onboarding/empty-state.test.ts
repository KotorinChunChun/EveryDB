import { beforeEach, describe, expect, it, vi } from "vitest";

import { mountEmptyState } from "./empty-state";

function mount(addSample = vi.fn(async () => {})) {
  document.body.innerHTML = `
    <section id="empty" hidden><button id="sample">サンプル</button><button id="add">追加</button><p id="error" hidden></p></section>
    <div id="grid"></div>`;
  const elements = {
    root: document.getElementById("empty")!,
    grid: document.getElementById("grid")!,
    sampleButton: document.getElementById("sample") as HTMLButtonElement,
    addButton: document.getElementById("add") as HTMLButtonElement,
    error: document.getElementById("error")!,
  };
  const openConnectionChooser = vi.fn();
  const controller = mountEmptyState(elements, { addSample, openConnectionChooser });
  return { controller, elements, addSample, openConnectionChooser };
}

describe("empty state", () => {
  beforeEach(() => { document.body.innerHTML = ""; });

  it("shows only while no sources exist", () => {
    const { controller, elements } = mount();
    controller.render(false);
    expect(elements.root.hidden).toBe(false);
    expect(elements.grid.hidden).toBe(true);
    controller.render(true);
    expect(elements.root.hidden).toBe(true);
    expect(elements.grid.hidden).toBe(false);
  });

  it("uses keyboard-accessible buttons for both actions", () => {
    const { elements, openConnectionChooser } = mount();
    expect(elements.sampleButton.tagName).toBe("BUTTON");
    expect(elements.addButton.tagName).toBe("BUTTON");
    elements.addButton.click();
    expect(openConnectionChooser).toHaveBeenCalledOnce();
  });

  it("keeps the recovery action visible when sample loading fails", async () => {
    const { controller, elements } = mount(vi.fn(async () => { throw new Error("CSVを開けません"); }));
    controller.render(false);
    elements.sampleButton.click();
    await vi.waitFor(() => expect(elements.error.hidden).toBe(false));
    expect(elements.root.hidden).toBe(false);
    expect(elements.error.textContent).toContain("CSVを開けません");
    expect(elements.sampleButton.disabled).toBe(false);
  });
});

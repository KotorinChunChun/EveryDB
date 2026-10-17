import { afterEach, describe, expect, it, vi } from "vitest";
import { createCsvPreviewController, renderCsvPreview } from "./csv-preview";

const sample = {
  columns: ["name", "value"],
  top_rows: [{ row_number: 2, values: ["A", "1"] }],
  bottom_rows: [{ row_number: 20, values: ["Z", "26"] }],
  total_rows: 19,
  encoding: "UTF-8",
  line_ending: "CRLF",
  delimiter: ",",
};

afterEach(() => vi.useRealTimers());

describe("CSV preview", () => {
  it("renders settings, edge rows and an omitted-row marker", () => {
    const host = document.createElement("div");
    host.innerHTML = '<div data-csv-preview-status></div><div data-csv-preview-body></div>';
    renderCsvPreview(host, sample);
    expect(host.textContent).toContain("19行 / UTF-8 / CRLF / 区切り: ,");
    expect(host.textContent).toContain("A");
    expect(host.textContent).toContain("Z");
    expect(host.textContent).toContain("17行省略");
  });

  it("debounces config changes and renders the newest processed result", async () => {
    vi.useFakeTimers();
    const request = vi.fn().mockResolvedValue(sample);
    const config = { path: "C:/data/sample.csv", delimiter: "," };
    const controller = createCsvPreviewController({ getConfig: () => config, request, delayMs: 250 });
    const host = document.createElement("div");
    controller.mount(host);
    controller.schedule();
    vi.advanceTimersByTime(249);
    expect(request).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(request).toHaveBeenCalledTimes(1);
    expect(host.textContent).toContain("19行");
  });
});

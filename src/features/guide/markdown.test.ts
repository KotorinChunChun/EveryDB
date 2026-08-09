import { describe, expect, it, vi } from "vitest";

import { renderMarkdownInto } from "./markdown";

vi.mock("../../app/ipc", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

describe("guide document navigation", () => {
  it("opens a relative guide link inside the guide viewer", () => {
    const container = document.createElement("div");
    const onOpenDocument = vi.fn();

    renderMarkdownInto(
      container,
      "[アプリケーション認証](microsoft365-entra-app.md)",
      { onOpenDocument },
    );
    container.querySelector("a")?.click();

    expect(onOpenDocument).toHaveBeenCalledWith("microsoft365-entra-app");
  });

  it("does not treat parent paths as in-app guide names", () => {
    const container = document.createElement("div");
    const onOpenDocument = vi.fn();

    renderMarkdownInto(container, "[別文書](../data-sources.md)", { onOpenDocument });
    container.querySelector("a")?.click();

    expect(onOpenDocument).not.toHaveBeenCalled();
  });
});

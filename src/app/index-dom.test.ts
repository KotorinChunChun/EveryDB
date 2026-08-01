import { describe, expect, it } from "vitest";

import html from "../../index.html?raw";
const requiredIds = [
  "app", "search", "sources", "status", "grid", "grid-head", "grid-viewport", "grid-rows", "result-info",
  "add-dialog", "parent-dialog", "column-map-dialog", "settings-dialog", "credential-manager",
  "guide-dialog", "confirm-dialog", "export-dialog", "shortcut-guide-dialog",
];

describe("index.html structural smoke test", () => {
  it("keeps all application IDs unique and preserves essential controls", () => {
    document.documentElement.innerHTML = html;
    const ids = [...document.querySelectorAll<HTMLElement>("[id]")].map((element) => element.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const id of requiredIds) expect(document.getElementById(id), `#${id}`).not.toBeNull();
  });

  it("gives every dialog a heading and retains the main dialog forms", () => {
    document.documentElement.innerHTML = html;
    const dialogs = [...document.querySelectorAll<HTMLDialogElement>("dialog")];
    expect(dialogs.length).toBeGreaterThanOrEqual(15);
    for (const dialog of dialogs) {
      expect(dialog.querySelector("h3"), `#${dialog.id} heading`).not.toBeNull();
    }
    expect(document.querySelector("#add-dialog form#add-form")).not.toBeNull();
    expect(document.querySelector("#export-dialog input[name='export-format']:checked")).not.toBeNull();
  });
});
import { describe, expect, it } from "vitest";

import html from "../../index.html?raw";
import { mountSimplePickerDialogs, SIMPLE_PICKER_DIALOGS } from "./simple-picker-dialogs";

const requiredIds = [
  "app", "search", "sources", "status", "grid", "grid-head", "grid-viewport", "grid-rows", "result-info",
  "add-dialog", "parent-dialog", "column-map-dialog", "settings-dialog", "credential-manager",
  "guide-dialog", "confirm-dialog", "url-auto-input-dialog", "export-dialog", "shortcut-guide-dialog",
  "empty-state", "empty-state-sample", "empty-state-add", "s-backup", "s-restore", "s-backup-status",
  "simple-picker-dialog-template",
  "connection-chooser-filter", "connection-chooser-filter-clear",
];

function mountAppDom(): void {
  document.documentElement.innerHTML = html;
  mountSimplePickerDialogs();
}

describe("index.html structural smoke test", () => {
  it("keeps all application IDs unique and preserves essential controls", () => {
    mountAppDom();
    const ids = [...document.querySelectorAll<HTMLElement>("[id]")].map((element) => element.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const id of requiredIds) expect(document.getElementById(id), `#${id}`).not.toBeNull();
    for (const def of SIMPLE_PICKER_DIALOGS) {
      expect(document.getElementById(def.dialogId), `#${def.dialogId}`).not.toBeNull();
    }
  });

  it("gives every dialog a heading and retains the main dialog forms", () => {
    mountAppDom();
    const dialogs = [...document.querySelectorAll<HTMLDialogElement>("dialog")];
    expect(dialogs.length).toBeGreaterThanOrEqual(15);
    for (const dialog of dialogs) {
      expect(dialog.querySelector("h3"), `#${dialog.id} heading`).not.toBeNull();
    }
    expect(document.querySelector("#add-dialog form#add-form")).not.toBeNull();
    expect(document.querySelector("#export-dialog input[name='export-format']:checked")).not.toBeNull();
  });

  it("labels every dialog for assistive technology", () => {
    mountAppDom();
    for (const dialog of document.querySelectorAll<HTMLDialogElement>("dialog")) {
      const labelledBy = dialog.getAttribute("aria-labelledby");
      expect(labelledBy, `#${dialog.id} aria-labelledby`).toBeTruthy();
      expect(document.getElementById(labelledBy!), `#${dialog.id} title #${labelledBy}`).not.toBeNull();
    }
  });

  it("names icon-only dialog close buttons", () => {
    mountAppDom();
    for (const button of document.querySelectorAll<HTMLButtonElement>(".dialog-close-x")) {
      expect(button.getAttribute("aria-label") || button.title, `#${button.id} accessible name`).toBeTruthy();
    }
  });

  it("mounts simple picker dialogs from shared template", () => {
    mountAppDom();
    for (const def of SIMPLE_PICKER_DIALOGS) {
      const dialog = document.getElementById(def.dialogId);
      expect(dialog?.querySelector(".picker-list"), `#${def.dialogId} picker list`).not.toBeNull();
      expect(document.getElementById(def.closeId), `#${def.closeId}`).not.toBeNull();
      expect(dialog?.getAttribute("aria-labelledby")).toBe(def.titleId);
    }
  });

  it("separates organizational and personal Microsoft credential scopes", () => {
    mountAppDom();
    const options = [...document.querySelectorAll<HTMLOptionElement>("#cm-scope option")]
      .map((option) => ({ value: option.value, label: option.textContent }));
    expect(options).toContainEqual({ value: "graph", label: "Microsoft Entra ID（組織）" });
    expect(options).toContainEqual({ value: "microsoft_account", label: "Microsoft アカウント（個人）" });
  });

  it("keeps every result-driven picker in a stable resizable frame", () => {
    mountAppDom();
    const ids = [
      "connection-chooser",
      "cell-picker",
      "table-picker",
      "sp-picker",
      "credential-select-picker",
      ...SIMPLE_PICKER_DIALOGS.map((definition) => definition.dialogId),
    ];
    for (const id of ids) {
      expect(document.getElementById(id)?.classList.contains("stable-picker-dialog"), id).toBe(true);
    }
  });
});

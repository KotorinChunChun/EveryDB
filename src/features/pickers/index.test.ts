import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import html from "../../../index.html?raw";
import { mountSimplePickerDialogs } from "../../app/simple-picker-dialogs";
import { mountSourcePickers } from "./index";

const invokeMock = vi.mocked(tauriInvoke);

function installDialogPolyfill(): void {
  Object.defineProperty(HTMLDialogElement.prototype, "showModal", {
    configurable: true,
    value(this: HTMLDialogElement) {
      this.setAttribute("open", "");
    },
  });
  Object.defineProperty(HTMLDialogElement.prototype, "close", {
    configurable: true,
    value(this: HTMLDialogElement) {
      this.removeAttribute("open");
      this.dispatchEvent(new Event("close"));
    },
  });
}

function mountPickers() {
  const sourceEditor = {
    getEditingId: () => null,
    getHiddenSharedValues: () => ({}),
    getConfigFromFields: () => ({ config: {}, missingText: false }),
    autoFillDefaultName: vi.fn(),
    maybeFetchAndRenderFields: vi.fn(),
  };
  const pickers = mountSourcePickers({
    getSourceEditor: () => sourceEditor as never,
    getParentSettings: () => ({ getSiblingSourceId: () => null, getParentSharedValues: () => ({}) }) as never,
    normalizePath: (value) => String(value ?? ""),
    loadSources: async () => {},
  });
  return { pickers, sourceEditor };
}

const flush = async () => {
  for (let i = 0; i < 5; i++) await Promise.resolve();
};

describe("source pickers", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    installDialogPolyfill();
    document.documentElement.innerHTML = html;
    mountSimplePickerDialogs();
    document.getElementById("f-dynamic")!.innerHTML =
      '<input id="fd-path" value="C:\\book.xlsx"><input id="fd-token" data-cred-ref="3">'
      + '<input id="fd-base_id"><input id="fd-base_name">';
  });

  it("fills the Airtable ID and caption, then refreshes the source editor", async () => {
    invokeMock.mockResolvedValue([{ id: "app1", name: "<b>顧客</b>", permission_level: "read" }] as never);
    const { pickers, sourceEditor } = mountPickers();
    const baseInput = document.getElementById("fd-base_id") as HTMLInputElement;

    pickers.openAirtableBasePicker(baseInput);
    await flush();

    expect(invokeMock).toHaveBeenCalledWith("browse_airtable_bases", { credentialId: 3 });
    const item = document.querySelector<HTMLElement>("#airtable-picker-list .picker-item")!;
    expect(item.textContent).toBe("<b>顧客</b> [ID: app1]");
    expect(item.querySelector("b")).toBeNull();
    item.click();

    expect(baseInput.value).toBe("app1");
    expect((document.getElementById("fd-base_name") as HTMLInputElement).value).toBe("<b>顧客</b>");
    expect(sourceEditor.autoFillDefaultName).toHaveBeenCalledOnce();
    expect(sourceEditor.maybeFetchAndRenderFields).toHaveBeenCalledOnce();
    expect((document.getElementById("airtable-picker") as HTMLDialogElement).open).toBe(false);
  });

  it("shows the empty message and the error text without interpreting markup", async () => {
    const { pickers } = mountPickers();
    const input = document.createElement("input");

    invokeMock.mockResolvedValueOnce([] as never);
    pickers.openSheetPicker(input);
    await flush();
    expect(document.getElementById("sheet-picker-title")?.textContent).toBe("シートを選択");
    expect(document.getElementById("sheet-picker-list")?.textContent).toBe("シートが見つかりませんでした");

    invokeMock.mockRejectedValueOnce("<img src=x>");
    pickers.openExcelTablePicker(input);
    await flush();
    const list = document.getElementById("sheet-picker-list")!;
    expect(document.getElementById("sheet-picker-title")?.textContent).toBe("テーブル(ListObject)を選択");
    expect(list.textContent).toBe("取得失敗: <img src=x>");
    expect(list.querySelector("img")).toBeNull();
  });
});

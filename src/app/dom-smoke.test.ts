import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import html from "../../index.html?raw";
import { confirmDelete, promptUrlInput } from "./dom";
import { appState } from "./state";
import { mountSimplePickerDialogs } from "./simple-picker-dialogs";
import { mountConnectionChooser } from "../features/source-editor/connection-chooser";
import { CONNECTION_KIND_OPTIONS } from "../features/source-editor/connection-kinds";
import { mountSearchControllerFromDom } from "../features/search/controller";
import { mountGridView } from "../features/grid/view";

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

function mountAppDom(): void {
  document.documentElement.innerHTML = html;
  mountSimplePickerDialogs();
}

function resetAppState(): void {
  appState.sources = [];
  appState.selectedSourceIds = new Set();
  appState.rawRows = [];
  appState.rows = [];
  appState.columns = [];
  appState.searchSeq = 0;
  appState.lastAppliedSeq = -1;
  appState.selectedCell = null;
  appState.parentLabelOverrides = new Map();
}

describe("application DOM interaction smoke", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    installDialogPolyfill();
    mountAppDom();
    resetAppState();
  });

  it("opens, submits, and cancels shared confirmation dialogs", async () => {
    const urlResult = promptUrlInput("接続 URL");
    const urlDialog = document.getElementById("url-auto-input-dialog") as HTMLDialogElement;
    const urlInput = document.getElementById("url-auto-input-dialog-input") as HTMLInputElement;

    expect(urlDialog.open).toBe(true);
    expect(document.getElementById("url-auto-input-dialog-hint")?.textContent).toBe("接続 URL");
    urlInput.value = "  https://example.test/data  ";
    (document.getElementById("url-auto-input-dialog-ok") as HTMLButtonElement).click();

    await expect(urlResult).resolves.toBe("https://example.test/data");
    expect(urlDialog.open).toBe(false);

    const deleteResult = confirmDelete("削除しますか");
    expect(document.getElementById("confirm-dialog-message")?.textContent).toBe("削除しますか");
    (document.getElementById("confirm-dialog-cancel") as HTMLButtonElement).click();
    await expect(deleteResult).resolves.toBe(false);
  });

  it("opens the source connection chooser and dispatches the selected kind", () => {
    const openAddDialog = vi.fn();
    const openParentSettingsDialog = vi.fn(async () => {});

    mountConnectionChooser({ openAddDialog, openParentSettingsDialog });
    (document.getElementById("add") as HTMLButtonElement).click();

    const chooser = document.getElementById("connection-chooser") as HTMLDialogElement;
    expect(chooser.open).toBe(true);
    const notion = [...chooser.querySelectorAll<HTMLButtonElement>(".connection-kind-item")]
      .find((button) => button.textContent === "Notion データベース");
    expect(notion).toBeDefined();
    notion!.click();

    expect(chooser.open).toBe(false);
    expect(openAddDialog).toHaveBeenCalledWith({
      type: "notion",
      title: "接続先を追加",
      connectionAdd: true,
    });
  });

  it("filters the connection chooser incrementally and clears the query", () => {
    mountConnectionChooser({ openAddDialog: vi.fn(), openParentSettingsDialog: vi.fn(async () => {}) });
    (document.getElementById("add") as HTMLButtonElement).click();

    const filter = document.getElementById("connection-chooser-filter") as HTMLInputElement;
    filter.value = "ブックマーク";
    filter.dispatchEvent(new Event("input"));
    expect([...document.querySelectorAll<HTMLElement>(".connection-kind-item")].map((item) => item.textContent))
      .toEqual(["ブラウザ: ブックマーク"]);
    expect([...document.querySelectorAll<HTMLElement>(".connection-kind-group")].map((item) => item.textContent))
      .toEqual(["ローカルファイル・PC"]);

    (document.getElementById("connection-chooser-filter-clear") as HTMLButtonElement).click();
    expect(filter.value).toBe("");
    expect(document.querySelectorAll(".connection-kind-item").length).toBe(CONNECTION_KIND_OPTIONS.length);
  });

  it("offers CSV and both OneDrive types as direct choices", () => {
    const openAddDialog = vi.fn();
    const openParentSettingsDialog = vi.fn(async () => {});

    mountConnectionChooser({ openAddDialog, openParentSettingsDialog });
    const chooser = document.getElementById("connection-chooser") as HTMLDialogElement;
    const choices = [...chooser.querySelectorAll<HTMLButtonElement>(".connection-kind-item")];

    expect(choices.some((button) => button.textContent === "CSV ファイル")).toBe(true);
    expect(choices.some((button) => button.textContent === "OneDrive（組織）")).toBe(true);
    expect(choices.some((button) => button.textContent === "OneDrive（個人）")).toBe(true);

    choices.find((button) => button.textContent === "CSV ファイル")!.click();
    expect(openAddDialog).toHaveBeenLastCalledWith({
      type: "csv",
      title: "テーブル追加",
      connectionAdd: false,
    });

    choices.find((button) => button.textContent === "OneDrive（個人）")!.click();
    expect(openAddDialog).toHaveBeenLastCalledWith({
      type: "microsoft_account_onedrive",
      title: "テーブル追加",
      connectionAdd: false,
    });
  });

  it("opens the OneDrive form even when a Microsoft 365 connection already exists", () => {
    appState.sources = [{
      id: 1,
      type_: "graph_entra_users",
      name: "ユーザー",
      parent: "Microsoft 365",
      refresh_interval: 60,
      status: null,
      last_success_at: null,
      last_error: null,
      record_count: 0,
    }];
    const openAddDialog = vi.fn();
    const openParentSettingsDialog = vi.fn(async () => {});

    mountConnectionChooser({ openAddDialog, openParentSettingsDialog });
    const oneDrive = [...document.querySelectorAll<HTMLButtonElement>(".connection-kind-item")]
      .find((button) => button.textContent === "OneDrive（組織）");
    oneDrive!.click();

    expect(openParentSettingsDialog).not.toHaveBeenCalled();
    expect(openAddDialog).toHaveBeenCalledWith({
      type: "graph_onedrive",
      title: "接続先を追加",
      connectionAdd: true,
    });
  });

  it("runs a search from mounted DOM and renders its response state", async () => {
    appState.selectedSourceIds = new Set([7]);
    invokeMock.mockResolvedValue(undefined);
    invokeMock.mockResolvedValueOnce({
      seq: 1,
      rows: [{
        source_id: 7,
        source_name: "顧客",
        label: "田中",
        open_ref: null,
        fields: { 氏名: "田中" },
      }],
      truncated: false,
    } as never);

    const onAfterSearch = vi.fn();
    const controller = mountSearchControllerFromDom({
      onAfterSearch,
      renderSources: vi.fn(),
    });
    (document.getElementById("search") as HTMLInputElement).value = "田中";

    await controller.runSearch();

    expect(invokeMock).toHaveBeenCalledWith("search", {
      query: "田中",
      sourceIds: [7],
      seq: 1,
      limit: 10000,
    });
    expect(appState.rows).toHaveLength(1);
    expect(document.getElementById("result-info")?.textContent).toBe("1 件");
    expect(onAfterSearch).toHaveBeenCalledOnce();
  });

  it("renders grid rows and moves the selected cell by keyboard", () => {
    appState.columns = ["#", "氏名"];
    appState.rows = [{
      source_id: 7,
      source_name: "顧客",
      label: "田中",
      open_ref: null,
      fields: { 氏名: "田中" },
    }];

    const controller = mountGridView({
      viewportEl: document.getElementById("grid-viewport") as HTMLDivElement,
      spacerEl: document.getElementById("grid-spacer") as HTMLDivElement,
      rowsEl: document.getElementById("grid-rows") as HTMLDivElement,
      headEl: document.getElementById("grid-head") as HTMLDivElement,
      gridRoot: document.getElementById("grid") as HTMLDivElement,
    }, {
      closeAllContextMenus: vi.fn(),
      openGridContextMenu: vi.fn(),
      totalGridWidth: () => 400,
      onRowsRendered: vi.fn(),
    });

    controller.renderRows();
    const cells = document.querySelectorAll<HTMLDivElement>("#grid-rows .cell");
    expect([...cells].map((cell) => cell.textContent)).toEqual(["1", "田中"]);

    cells[0].dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
    expect(appState.selectedCell).toEqual({ row: 0, col: 0 });

    document.getElementById("grid")!.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true, cancelable: true }),
    );
    expect(appState.selectedCell).toEqual({ row: 0, col: 1 });
    expect(document.querySelector("#grid-rows .cell.sel")?.textContent).toBe("田中");
  });
});

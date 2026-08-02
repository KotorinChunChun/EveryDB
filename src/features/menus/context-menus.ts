import { confirmDelete, positionContextMenu } from "../../app/dom";
import { invoke } from "../../app/ipc";
import { appState, clearContextMenuTargets } from "../../app/state";
import type { Row, ReloadSummary, SourceDetail } from "../../shared/types";
import type { ParentKind } from "../../source-types/registry";
import { canOpenTable } from "./openable";

export interface ContextMenuElements {
  sourceMenu: HTMLDivElement;
  gridMenu: HTMLDivElement;
  parentMenu: HTMLDivElement;
}

export interface ContextMenuActions {
  parentLabel: (parentKey: string) => string;
  parentKindOf: (type: string) => ParentKind;
  reloadSummaryMessage: (summary: ReloadSummary) => string;
  cellText: (row: Row, col: number) => string;
  getCellUrl: (text: string) => string | null;
  openEditDialog: (id: number) => Promise<void>;
  openParentSettingsDialog: (parentKey: string) => Promise<void>;
  openExportDialog: () => void;
  loadSources: () => Promise<void>;
  renderSources: () => void;
  runSearch: () => Promise<void>;
  reloadOne: (id: number) => Promise<void>;
  setResultInfo: (text: string) => void;
  copySelected: () => Promise<void>;
  copyRowAsTsv: () => Promise<void>;
  copyRowAsTsvWithHeader: () => Promise<void>;
  copyRowAsJson: () => Promise<void>;
}

export interface ContextMenuController {
  openContextMenu: (ev: MouseEvent, id: number) => void;
  openGridContextMenu: (ev: MouseEvent) => void;
  openParentContextMenu: (ev: MouseEvent, parentKey: string) => void;
  closeContextMenu: () => void;
  closeAllContextMenus: () => void;
}

async function openTableBySourceId(
  id: number,
  setResultInfo: (text: string) => void,
  tableOpenUrl: (type: string, config: Record<string, unknown>) => string | null,
): Promise<void> {
  try {
    const detail = await invoke<SourceDetail>("get_source_detail", { id });
    const url = tableOpenUrl(detail.type_, detail.config);
    if (url) {
      await invoke("open_ref", { target: url });
    } else {
      setResultInfo("このテーブルは直接開けません（接続情報が不足しています）");
    }
  } catch (e) {
    setResultInfo("開けませんでした: " + String(e));
  }
}

export function mountContextMenus(
  elements: ContextMenuElements,
  actions: ContextMenuActions,
  tableOpenUrl: (type: string, config: Record<string, unknown>) => string | null,
): ContextMenuController {
  const { sourceMenu, gridMenu, parentMenu } = elements;

  function closeContextMenu(): void {
    sourceMenu.hidden = true;
    clearContextMenuTargets();
  }

  function closeAllContextMenus(): void {
    sourceMenu.hidden = true;
    gridMenu.hidden = true;
    parentMenu.hidden = true;
    clearContextMenuTargets();
  }

  function openContextMenu(ev: MouseEvent, id: number): void {
    closeAllContextMenus();
    appState.ctxTargetId = id;
    const src = appState.sources.find((s) => s.id === id);
    const openTableBtn = sourceMenu.querySelector<HTMLButtonElement>('[data-action="open-table"]')!;
    openTableBtn.hidden = !(src && canOpenTable(src.type_));
    positionContextMenu(sourceMenu, ev.clientX, ev.clientY);
  }

  function openGridContextMenu(ev: MouseEvent): void {
    closeAllContextMenus();
    const row = appState.selectedCell ? appState.rows[appState.selectedCell.row] : undefined;
    const openPageBtn = gridMenu.querySelector<HTMLButtonElement>('[data-action="open-page"]');
    if (openPageBtn) {
      const url = row && appState.selectedCell
        ? actions.getCellUrl(actions.cellText(row, appState.selectedCell.col))
        : null;
      openPageBtn.disabled = !url;
    }
    const openTableBtn = gridMenu.querySelector<HTMLButtonElement>('[data-action="open-table"]');
    if (openTableBtn) {
      const src = row ? appState.sources.find((s) => s.id === row.source_id) : undefined;
      openTableBtn.disabled = !(src && canOpenTable(src.type_));
    }
    positionContextMenu(gridMenu, ev.clientX, ev.clientY);
  }

  function openParentContextMenu(ev: MouseEvent, parentKey: string): void {
    closeAllContextMenus();
    appState.parentCtxTargetKey = parentKey;
    positionContextMenu(parentMenu, ev.clientX, ev.clientY);
  }

  document.addEventListener("click", () => closeAllContextMenus());
  document.addEventListener("contextmenu", (ev) => {
    const target = ev.target as HTMLElement;
    if (!target.closest(".source-item") && !target.closest(".cell") && !target.closest(".tree-parent")) {
      closeAllContextMenus();
    }
  });
  document.addEventListener("keydown", (ev) => {
    if (ev.key === "Escape") closeAllContextMenus();
  });
  window.addEventListener("blur", () => closeAllContextMenus());

  gridMenu.addEventListener("click", async (ev) => {
    const btn = (ev.target as HTMLElement).closest("button");
    if (!btn) return;
    closeAllContextMenus();
    if (btn.dataset.copy) {
      switch (btn.dataset.copy) {
        case "value": await actions.copySelected(); break;
        case "tsv": await actions.copyRowAsTsv(); break;
        case "tsv-header": await actions.copyRowAsTsvWithHeader(); break;
        case "json": await actions.copyRowAsJson(); break;
      }
    } else if (btn.dataset.action === "open-table") {
      if (!appState.selectedCell) return;
      const row = appState.rows[appState.selectedCell.row];
      if (!row) return;
      await openTableBySourceId(row.source_id, actions.setResultInfo, tableOpenUrl);
    } else if (btn.dataset.action === "open-page") {
      if (!appState.selectedCell || btn.disabled) return;
      const row = appState.rows[appState.selectedCell.row];
      if (!row) return;
      const url = actions.getCellUrl(actions.cellText(row, appState.selectedCell.col));
      if (url) {
        try {
          await invoke("open_ref", { target: url });
        } catch (e) {
          actions.setResultInfo("ページを開けませんでした: " + String(e));
        }
      }
    } else if (btn.dataset.action === "hide-source") {
      if (!appState.selectedCell) return;
      const row = appState.rows[appState.selectedCell.row];
      if (!row) return;
      appState.selectedSourceIds.delete(row.source_id);
      actions.renderSources();
      await actions.runSearch();
    }
  });

  sourceMenu.addEventListener("click", async (ev) => {
    const btn = (ev.target as HTMLElement).closest("button");
    if (!btn || appState.ctxTargetId === null) return;
    const id = appState.ctxTargetId;
    const src = appState.sources.find((s) => s.id === id);
    closeContextMenu();
    switch (btn.dataset.action) {
      case "view":
        await actions.openEditDialog(id);
        break;
      case "duplicate": {
        const name = prompt("複製後の表示名", `${src?.name ?? ""} のコピー`);
        if (!name) break;
        await invoke("duplicate_source", { id, newName: name });
        await actions.loadSources();
        break;
      }
      case "reload":
        await actions.reloadOne(id);
        break;
      case "open-table":
        await openTableBySourceId(id, actions.setResultInfo, tableOpenUrl);
        break;
      case "export":
        appState.selectedSourceIds = new Set([id]);
        actions.renderSources();
        await actions.runSearch();
        actions.openExportDialog();
        break;
      case "delete":
        if (!await confirmDelete(`「${src?.name ?? ""}」を削除しますか？\n（キャッシュも消えます）`)) break;
        await invoke("delete_source", { id });
        appState.selectedSourceIds.delete(id);
        await actions.loadSources();
        break;
    }
  });

  parentMenu.addEventListener("click", async (ev) => {
    const btn = (ev.target as HTMLElement).closest("button");
    const parentKey = appState.parentCtxTargetKey;
    if (!btn || parentKey === null) return;
    closeAllContextMenus();
    const children = appState.sources.filter((s) => s.parent === parentKey);
    if (children.length === 0) return;
    switch (btn.dataset.action) {
      case "edit":
        await actions.openParentSettingsDialog(parentKey);
        break;
      case "reload": {
        actions.setResultInfo("リロード中…");
        try {
          const summary = await invoke<ReloadSummary>("reload_sources", { sourceIds: children.map((c) => c.id) });
          actions.setResultInfo(actions.reloadSummaryMessage(summary));
        } catch (e) {
          actions.setResultInfo("リロード失敗: " + String(e));
        }
        await actions.loadSources();
        await actions.runSearch();
        break;
      }
      case "open": {
        const kind = actions.parentKindOf(children[0].type_);
        if (kind !== "path" && kind !== "sharepoint") {
          actions.setResultInfo("この接続先は直接開けません");
          break;
        }
        try {
          await invoke("open_ref", { target: parentKey });
        } catch (e) {
          actions.setResultInfo("開けませんでした: " + String(e));
        }
        break;
      }
      case "export":
        appState.selectedSourceIds = new Set(children.map((c) => c.id));
        actions.renderSources();
        await actions.runSearch();
        actions.openExportDialog();
        break;
      case "delete": {
        const names = children.map((c) => c.name).join(", ");
        if (!await confirmDelete(
          `「${actions.parentLabel(parentKey)}」配下の ${children.length} 個のテーブルをすべて削除しますか？\n（${names}）\nキャッシュも消えます。`,
        )) break;
        for (const child of children) {
          await invoke("delete_source", { id: child.id });
          appState.selectedSourceIds.delete(child.id);
        }
        await actions.loadSources();
        break;
      }
    }
  });

  return {
    openContextMenu,
    openGridContextMenu,
    openParentContextMenu,
    closeContextMenu,
    closeAllContextMenus,
  };
}

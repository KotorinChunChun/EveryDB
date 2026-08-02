import { confirmDelete } from "../../app/dom";
import { invoke } from "../../app/ipc";
import {
  appState,
  saveCollapsedParentsState,
  saveSelectedSourcesState,
} from "../../app/state";
import type { ReloadSummary, SourceInfo } from "../../shared/types";
import {
  collapseAllParents,
  expandAllParents,
  showChildrenWhileFiltered,
  toggleParentCollapsed,
} from "./collapse";
import {
  clearSourceDragSession,
  clearSourceDropHighlight,
  computeReorderIds,
  createSourceDragSession,
  createSourceDropHighlight,
  isVerticalDropAfter,
  resolveSourceDropTarget,
  startParentDrag,
  startSourceItemDrag,
  updateSourceDropHighlight,
} from "./drag-drop";
import { manageModeSidebarWidth } from "./manage-mode";
import { reorderedSourceIds } from "./reorder";
import {
  setVisibleSelection,
  toggleParentSelection,
  toggleSourceSelection,
} from "./selection";
import {
  parentLabel as formatParentLabel,
  shortTimestamp,
  sourceMatchesFilter as matchesSourceFilter,
  sourceTree,
  sourceTypeLabel,
} from "./tree";

const MANAGE_MIN_W = 720;

export interface SourcesSidebarElements {
  sourcesEl: HTMLElement;
  statusEl: HTMLElement;
  sidebarEl: HTMLElement;
  manageToggleBtn: HTMLButtonElement;
  manageHeadEl: HTMLElement;
  sourceFilterInput: HTMLInputElement;
  sourceFilterClearBtn: HTMLButtonElement;
  selectAllBtn: HTMLButtonElement;
  selectNoneBtn: HTMLButtonElement;
  expandAllBtn: HTMLButtonElement;
  collapseAllBtn: HTMLButtonElement;
  statusErrorsCloseBtn: HTMLButtonElement;
  statusErrorsCloseXBtn: HTMLButtonElement;
  statusErrorsDialog: HTMLDialogElement;
  statusErrorsList: HTMLDivElement;
}

export interface SourcesSidebarActions {
  onSearch: () => void;
  loadSources: () => Promise<void>;
  refreshSidebar: () => Promise<void>;
  openEditDialog: (id: number) => void;
  openParentSettingsDialog: (parentKey: string) => void;
  openContextMenu: (ev: MouseEvent, id: number) => void;
  openParentContextMenu: (ev: MouseEvent, parentKey: string) => void;
  openExportDialog: () => void;
  setResultInfo: (text: string) => void;
}

export interface SourcesSidebarController {
  renderSources: () => void;
  setManageMode: (on: boolean) => void;
  visibleSources: () => SourceInfo[];
  reloadOne: (id: number) => Promise<void>;
}

function reloadSummaryMessage(summary: ReloadSummary): string {
  const counts = `成功 ${summary.successful} 件、失敗 ${summary.failed} 件、取消 ${summary.cancelled} 件`;
  const firstFailure = summary.results.find((result) => result.outcome === "failed")?.error;
  return firstFailure ? `リロード完了: ${counts}。${firstFailure}` : `リロード完了: ${counts}`;
}

export function mountSourcesSidebar(
  elements: SourcesSidebarElements,
  actions: SourcesSidebarActions,
): SourcesSidebarController {
  const dragSession = createSourceDragSession();
  const dropHighlight = createSourceDropHighlight();
  const collapsedParents = appState.collapsedParents;

  function parentLabel(parent: string): string {
    return formatParentLabel(parent, appState.parentLabelOverrides);
  }

  function sourceMatchesFilter(source: SourceInfo): boolean {
    return matchesSourceFilter(source, appState.sourceFilter, appState.parentLabelOverrides);
  }

  function visibleSources(): SourceInfo[] {
    return appState.sources.filter(sourceMatchesFilter);
  }

  async function performReorder(
    targetParentKey: string | null,
    targetId: number | null,
    insertAfter = false,
  ): Promise<void> {
    const orderedIds = computeReorderIds(
      appState.sources,
      dragSession,
      targetParentKey,
      targetId,
      insertAfter,
    );
    if (!orderedIds) return;
    await invoke("reorder_sources", { orderedIds });
    await actions.refreshSidebar();
  }

  function clearDropTargets(): void {
    clearSourceDropHighlight(dropHighlight);
  }

  function handleDragOver(ev: DragEvent, el: HTMLElement): boolean {
    ev.preventDefault();
    ev.stopPropagation();
    const targetEl = resolveSourceDropTarget(el, dragSession.dragParentKey);
    const rect = targetEl.getBoundingClientRect();
    const isAfter = isVerticalDropAfter(ev.clientY, rect.top, rect.height);
    updateSourceDropHighlight(dropHighlight, targetEl, isAfter);
    return isAfter;
  }

  function makeManageButton(
    text: string,
    title: string,
    onClick: () => void | Promise<void>,
    danger = false,
  ): HTMLButtonElement {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "m-btn" + (danger ? " danger" : "");
    btn.textContent = text;
    btn.title = title;
    btn.onclick = async (ev) => {
      ev.stopPropagation();
      ev.preventDefault();
      await onClick();
    };
    return btn;
  }

  function makeIntervalInput(
    minutes: number | null,
    onCommit: (minutes: number) => Promise<void>,
    row: HTMLElement,
  ): HTMLInputElement {
    const input = document.createElement("input");
    input.type = "number";
    input.className = "m-interval";
    input.min = "1";
    input.value = minutes === null ? "" : String(minutes);
    if (minutes === null) input.placeholder = "―";
    input.title = minutes === null
      ? "配下のテーブルで間隔が揃っていません。入力すると全テーブルへ一括設定します"
      : "自動更新の間隔（分）";
    input.addEventListener("mousedown", (ev) => { ev.stopPropagation(); row.draggable = false; });
    input.addEventListener("blur", () => { row.draggable = true; });
    input.addEventListener("click", (ev) => ev.stopPropagation());
    input.addEventListener("dblclick", (ev) => ev.stopPropagation());
    input.onchange = async () => {
      const m = Math.max(1, parseInt(input.value, 10) || 1);
      input.value = String(m);
      try {
        await onCommit(m);
      } catch (e) {
        alert("更新間隔の設定に失敗: " + String(e));
      }
    };
    return input;
  }

  async function reloadOne(id: number): Promise<void> {
    actions.setResultInfo("リロード中…");
    try {
      const summary = await invoke<ReloadSummary>("reload_sources", { sourceIds: [id] });
      actions.setResultInfo(reloadSummaryMessage(summary));
    } catch (e) {
      actions.setResultInfo("リロード失敗: " + String(e));
    }
    await actions.refreshSidebar();
    actions.onSearch();
  }

  function makeSourceItem(source: SourceInfo): HTMLElement {
    const item = document.createElement("div");
    item.className = "source-item";
    item.draggable = true;
    item.dataset.id = String(source.id);

    const dot = document.createElement("span");
    dot.className = "dot " + (source.status === "ok" ? "ok" : source.status === "error" ? "err" : "idle");
    dot.title = source.last_error ?? source.status ?? "未更新";

    const cb = document.createElement("input");
    cb.type = "checkbox";
    cb.checked = appState.selectedSourceIds.has(source.id);
    cb.id = `src-${source.id}`;
    cb.onclick = (ev) => {
      appState.selectedSourceIds = toggleSourceSelection(
        appState.selectedSourceIds,
        source.id,
        cb.checked,
        (ev as MouseEvent).altKey,
      );
      saveSelectedSourcesState();
      renderSources();
      actions.onSearch();
    };

    const label = document.createElement("label");
    label.htmlFor = cb.id;
    label.textContent = source.name;
    label.title = source.name;

    const count = document.createElement("span");
    count.className = "count";
    count.textContent = String(source.record_count);

    const reloadBtn = document.createElement("button");
    reloadBtn.className = "mini-btn";
    reloadBtn.title = "このソースだけ再読込";
    reloadBtn.textContent = "⟳";
    reloadBtn.onclick = async (ev) => { ev.stopPropagation(); await reloadOne(source.id); };

    if (appState.manageMode) {
      const typeSpan = document.createElement("span");
      typeSpan.className = "m-type";
      typeSpan.textContent = sourceTypeLabel(source.type_);
      typeSpan.title = source.type_;

      count.classList.add("m-count");

      const lastSpan = document.createElement("span");
      lastSpan.className = "m-last" + (source.status === "error" ? " err" : "");
      lastSpan.textContent = shortTimestamp(source.last_success_at);
      lastSpan.title = source.status === "error"
        ? `最終成功: ${source.last_success_at ?? "なし"}\n直近の更新は失敗: ${source.last_error ?? ""}`
        : `最終成功: ${source.last_success_at ?? "なし"}`;

      const intervalInput = makeIntervalInput(
        Math.max(1, Math.round(source.refresh_interval / 60)),
        async (m) => {
          await invoke("set_refresh_interval", { id: source.id, seconds: m * 60 });
          source.refresh_interval = m * 60;
        },
        item,
      );

      const rowActions = document.createElement("span");
      rowActions.className = "m-actions";
      rowActions.append(
        makeManageButton("⟳", "このテーブルを再読込", () => reloadOne(source.id)),
        makeManageButton("✎", "このテーブルの設定を編集", () => actions.openEditDialog(source.id)),
        makeManageButton("🗑", "このテーブルを削除", async () => {
          if (!await confirmDelete(`「${source.name}」を削除しますか？\n（キャッシュも消えます）`)) return;
          await invoke("delete_source", { id: source.id });
          appState.selectedSourceIds.delete(source.id);
          await actions.loadSources();
        }, true),
      );

      item.append(dot, cb, label, typeSpan, count, lastSpan, intervalInput, rowActions);
    } else {
      item.append(dot, cb, label, count, reloadBtn);
    }

    item.addEventListener("contextmenu", (ev) => {
      ev.preventDefault();
      actions.openContextMenu(ev as MouseEvent, source.id);
    });

    item.addEventListener("dragstart", () => {
      startSourceItemDrag(dragSession, source.id);
      item.classList.add("dragging");
    });
    item.addEventListener("dragend", () => {
      item.classList.remove("dragging");
      clearSourceDragSession(dragSession);
      clearDropTargets();
    });
    item.addEventListener("dragover", (ev) => { handleDragOver(ev as DragEvent, item); });
    item.addEventListener("drop", async (ev) => {
      ev.preventDefault();
      ev.stopPropagation();
      const targetEl = resolveSourceDropTarget(item, dragSession.dragParentKey);
      const rect = targetEl.getBoundingClientRect();
      const isAfter = isVerticalDropAfter((ev as MouseEvent).clientY, rect.top, rect.height);
      clearDropTargets();
      if (dragSession.dragParentKey !== null) {
        const group = item.closest<HTMLElement>(".tree-group");
        const parentKey = group ? group.dataset.parentKey ?? null : null;
        if (parentKey) await performReorder(parentKey, null, isAfter);
      } else {
        await performReorder(null, source.id, isAfter);
      }
    });
    return item;
  }

  function openStatusErrorsDialog(errs: SourceInfo[]): void {
    elements.statusErrorsList.innerHTML = "";
    for (const source of errs) {
      const item = document.createElement("div");
      item.className = "status-error-item";
      const name = document.createElement("div");
      name.className = "status-error-name";
      name.textContent = source.name;
      const msg = document.createElement("div");
      msg.className = "status-error-msg";
      msg.textContent = source.last_error ?? "(エラー内容なし)";
      item.append(name, msg);
      elements.statusErrorsList.appendChild(item);
    }
    elements.statusErrorsDialog.showModal();
  }

  function updateStatus(): void {
    const errs = appState.sources.filter((s) => s.status === "error");
    if (errs.length) {
      elements.statusEl.innerHTML = "";
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "status-error-link";
      btn.textContent = `⚠ 更新エラー: ${errs.length}件（クリックで詳細）`;
      btn.onclick = () => openStatusErrorsDialog(errs);
      elements.statusEl.appendChild(btn);
    } else {
      const last = appState.sources.map((s) => s.last_success_at).filter(Boolean).sort().pop();
      elements.statusEl.textContent = last ? `最終更新: ${last}` : "まだ更新されていません";
    }
  }

  function renderSources(): void {
    saveSelectedSourcesState();
    elements.sourcesEl.innerHTML = "";
    const filterActive = appState.sourceFilter.trim().length > 0;
    const match = sourceMatchesFilter;

    for (const entry of sourceTree(appState.sources)) {
      if (entry.kind === "source") {
        if (match(entry.source)) elements.sourcesEl.appendChild(makeSourceItem(entry.source));
        continue;
      }
      const children = entry.children;
      const head = children[0];
      const visibleChildren = children.filter(match);
      if (visibleChildren.length === 0) continue;

      const selCount = children.filter((c) => appState.selectedSourceIds.has(c.id)).length;
      const collapsed = collapsedParents.has(head.parent);

      const parentRow = document.createElement("div");
      parentRow.className = "tree-parent";

      const twisty = document.createElement("span");
      twisty.className = "twisty";
      twisty.textContent = collapsed ? "▶" : "▼";
      const toggleCollapse = (ev?: Event) => {
        if (ev) ev.stopPropagation();
        toggleParentCollapsed(collapsedParents, head.parent);
        saveCollapsedParentsState();
        renderSources();
      };
      twisty.onclick = toggleCollapse;

      const pcb = document.createElement("input");
      pcb.type = "checkbox";
      pcb.checked = selCount === children.length;
      pcb.indeterminate = selCount > 0 && selCount < children.length;
      const toggleCheck = (ev?: MouseEvent | Event) => {
        if (ev) ev.stopPropagation();
        const isAlt = ev && "altKey" in ev && (ev as MouseEvent).altKey;
        appState.selectedSourceIds = toggleParentSelection(
          appState.selectedSourceIds,
          children.map((child) => child.id),
          Boolean(isAlt),
        );
        saveSelectedSourcesState();
        renderSources();
        actions.onSearch();
      };
      pcb.onclick = (ev) => toggleCheck(ev);

      const plabel = document.createElement("span");
      plabel.className = "p-label";
      plabel.textContent = parentLabel(head.parent);
      plabel.title = `${head.parent}\n(${children.length} テーブル)`;

      const groupWrap = document.createElement("div");
      groupWrap.className = "tree-group";
      groupWrap.dataset.parentKey = head.parent;

      const parentReloadBtn = document.createElement("button");
      parentReloadBtn.className = "mini-btn";
      parentReloadBtn.title = "この接続先のテーブルをすべて再読込";
      parentReloadBtn.textContent = "⟳";
      parentReloadBtn.onclick = async (ev) => {
        ev.stopPropagation();
        actions.setResultInfo("リロード中…");
        try {
          const summary = await invoke<ReloadSummary>("reload_sources", { sourceIds: children.map((c) => c.id) });
          actions.setResultInfo(reloadSummaryMessage(summary));
        } catch (e) {
          actions.setResultInfo("リロード失敗: " + String(e));
        }
        await actions.refreshSidebar();
        actions.onSearch();
      };

      if (!appState.manageMode) {
        parentRow.append(twisty, pcb, plabel, parentReloadBtn);
      } else {
        parentRow.append(twisty, pcb, plabel);
        const parentKey = head.parent;
        const totalRecords = children.reduce((a, c) => a + c.record_count, 0);
        const lastSuccess = children.map((c) => c.last_success_at).filter(Boolean).sort().pop() ?? null;
        const hasError = children.some((c) => c.status === "error");
        const intervals = new Set(children.map((c) => c.refresh_interval));
        const commonMinutes = intervals.size === 1 ? Math.max(1, Math.round([...intervals][0] / 60)) : null;

        const ptype = document.createElement("span");
        ptype.className = "m-type";
        ptype.textContent = sourceTypeLabel(children[0].type_);
        ptype.title = `${children.length} 個のテーブル: ${children.map((c) => c.name).join(", ")}`;

        const ptotal = document.createElement("span");
        ptotal.className = "count m-count total";
        ptotal.textContent = String(totalRecords);
        ptotal.title = `配下 ${children.length} テーブルの合計件数`;

        const plast = document.createElement("span");
        plast.className = "m-last" + (hasError ? " err" : "");
        plast.textContent = shortTimestamp(lastSuccess);
        plast.title = hasError
          ? `配下に更新失敗中のテーブルがあります（最新の成功: ${lastSuccess ?? "なし"}）`
          : `配下で最も新しい成功: ${lastSuccess ?? "なし"}`;

        const pinterval = makeIntervalInput(commonMinutes, async (m) => {
          for (const child of children) {
            await invoke("set_refresh_interval", { id: child.id, seconds: m * 60 });
            child.refresh_interval = m * 60;
          }
          renderSources();
        }, parentRow);

        const pactions = document.createElement("span");
        pactions.className = "m-actions";
        pactions.append(
          makeManageButton("⟳", "配下のテーブルをすべて再読込", async () => {
            actions.setResultInfo("リロード中…");
            try {
              const summary = await invoke<ReloadSummary>("reload_sources", { sourceIds: children.map((c) => c.id) });
              actions.setResultInfo(reloadSummaryMessage(summary));
            } catch (e) {
              actions.setResultInfo("リロード失敗: " + String(e));
            }
            await actions.refreshSidebar();
            actions.onSearch();
          }),
          makeManageButton("⚙", "この接続先の接続設定を編集", () => actions.openParentSettingsDialog(parentKey)),
          makeManageButton("🗑", "配下のテーブルをすべて削除", async () => {
            const names = children.map((c) => c.name).join("、");
            if (!await confirmDelete(
              `「${parentLabel(parentKey)}」配下の ${children.length} 個のテーブルをすべて削除しますか？\n（${names}）\nキャッシュも消えます。`,
            )) return;
            for (const child of children) {
              await invoke("delete_source", { id: child.id });
              appState.selectedSourceIds.delete(child.id);
            }
            await actions.loadSources();
          }, true),
        );

        parentRow.append(ptype, ptotal, plast, pinterval, pactions);
      }
      parentRow.draggable = true;

      let clickTimer: ReturnType<typeof setTimeout> | null = null;
      parentRow.onclick = (ev) => {
        if (ev.target === twisty || ev.target === pcb) return;
        if (clickTimer) clearTimeout(clickTimer);
        const mouseEv = ev;
        clickTimer = setTimeout(() => {
          clickTimer = null;
          toggleCheck(mouseEv);
        }, 180);
      };
      parentRow.ondblclick = (ev) => {
        if (ev.target === twisty || ev.target === pcb) return;
        if (clickTimer) {
          clearTimeout(clickTimer);
          clickTimer = null;
        }
        toggleCollapse();
      };
      parentRow.addEventListener("contextmenu", (ev) => {
        ev.preventDefault();
        actions.openParentContextMenu(ev as MouseEvent, head.parent);
      });
      parentRow.addEventListener("dragstart", () => {
        startParentDrag(dragSession, head.parent);
        groupWrap.classList.add("dragging");
      });
      parentRow.addEventListener("dragend", () => {
        groupWrap.classList.remove("dragging");
        clearSourceDragSession(dragSession);
        clearDropTargets();
      });
      parentRow.addEventListener("dragover", (ev) => { handleDragOver(ev as DragEvent, parentRow); });
      parentRow.addEventListener("drop", async (ev) => {
        ev.preventDefault();
        ev.stopPropagation();
        const targetEl = resolveSourceDropTarget(parentRow, dragSession.dragParentKey);
        const rect = targetEl.getBoundingClientRect();
        const isAfter = isVerticalDropAfter((ev as MouseEvent).clientY, rect.top, rect.height);
        clearDropTargets();
        await performReorder(head.parent, null, isAfter);
      });

      const childWrap = document.createElement("div");
      childWrap.className = "tree-children";
      childWrap.hidden = !showChildrenWhileFiltered(collapsed, filterActive);
      for (const child of visibleChildren) childWrap.appendChild(makeSourceItem(child));

      groupWrap.append(parentRow, childWrap);
      elements.sourcesEl.appendChild(groupWrap);
    }
    updateStatus();
  }

  function setManageMode(on: boolean): void {
    appState.manageMode = on;
    elements.sidebarEl.classList.toggle("manage", on);
    elements.manageToggleBtn.classList.toggle("active", on);
    elements.manageToggleBtn.setAttribute("aria-pressed", String(on));
    elements.manageHeadEl.hidden = !on;
    if (on) {
      appState.widthBeforeManage = Math.round(elements.sidebarEl.getBoundingClientRect().width);
      elements.sidebarEl.style.width = manageModeSidebarWidth(
        appState.widthBeforeManage,
        window.innerWidth,
        MANAGE_MIN_W,
      ) + "px";
    } else if (appState.widthBeforeManage !== null) {
      elements.sidebarEl.style.width = appState.widthBeforeManage + "px";
      appState.widthBeforeManage = null;
    }
    renderSources();
  }

  function setSourceFilter(value: string): void {
    appState.sourceFilter = value;
    elements.sourceFilterInput.value = value;
    elements.sourceFilterClearBtn.hidden = value === "";
    renderSources();
  }

  elements.manageToggleBtn.onclick = () => setManageMode(!appState.manageMode);
  elements.sourceFilterInput.addEventListener("input", (e) => setSourceFilter((e.target as HTMLInputElement).value));
  elements.sourceFilterInput.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && elements.sourceFilterInput.value !== "") setSourceFilter("");
  });
  elements.sourceFilterClearBtn.onclick = () => {
    setSourceFilter("");
    elements.sourceFilterInput.focus();
  };
  elements.selectAllBtn.onclick = () => {
    appState.selectedSourceIds = setVisibleSelection(
      appState.selectedSourceIds,
      visibleSources().map((source) => source.id),
      true,
    );
    saveSelectedSourcesState();
    renderSources();
    actions.onSearch();
  };
  elements.selectNoneBtn.onclick = () => {
    appState.selectedSourceIds = setVisibleSelection(
      appState.selectedSourceIds,
      visibleSources().map((source) => source.id),
      false,
    );
    saveSelectedSourcesState();
    renderSources();
    actions.onSearch();
  };
  elements.expandAllBtn.onclick = () => {
    expandAllParents(collapsedParents);
    saveCollapsedParentsState();
    renderSources();
  };
  elements.collapseAllBtn.onclick = () => {
    collapseAllParents(collapsedParents, appState.sources);
    saveCollapsedParentsState();
    renderSources();
  };
  elements.statusErrorsCloseBtn.onclick = () => elements.statusErrorsDialog.close();
  elements.statusErrorsCloseXBtn.onclick = () => elements.statusErrorsDialog.close();
  elements.sourcesEl.addEventListener("dragleave", (ev) => {
    if (!elements.sourcesEl.contains(ev.relatedTarget as Node)) clearDropTargets();
  });

  return { renderSources, setManageMode, visibleSources, reloadOne };
}

// re-export for tests that assert reorder wiring
export { reorderedSourceIds };

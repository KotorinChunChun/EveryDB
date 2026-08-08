import { invoke } from "../../app/ipc";
import {
  appState,
  applyGridSort,
  clearSelectedCell,
  rebuildGridColumns,
} from "../../app/state";
import {
  clearColumnDropHighlight,
  createColumnDropHighlight,
  handleColumnDragOver,
} from "./column-drag";
import {
  buildSourceFieldIndex,
  gridColumnWidth,
  nextSortState,
  PINNED_GRID_COLUMNS,
  resizedGridColumnWidth,
  sortHintFor as gridSortHint,
  toRawFieldNames,
} from "./data";
import { reorderDisplayColumns, sourceColumnOrderForDisplay } from "./column-order";

const DEFAULT_COL_W = 200;

export interface GridHeadElements {
  headEl: HTMLDivElement;
  viewportEl: HTMLDivElement;
}

export interface GridHeadActions {
  totalGridWidth: () => number;
  renderRows: () => void;
  refreshSidebar: () => Promise<void>;
  setResultInfo: (text: string) => void;
  openColumnSettings: (columnName: string) => Promise<void>;
  clearDropTargets: () => void;
}

export interface GridHeadController {
  renderHead: () => void;
}

export function mountGridHead(
  elements: GridHeadElements,
  actions: GridHeadActions,
): GridHeadController {
  const { headEl, viewportEl } = elements;
  const columnDropHighlight = createColumnDropHighlight();
  let gridDragSrcCol: string | null = null;
  let suppressHeadClick = false;

  function colWidth(name: string): number {
    return gridColumnWidth(name, appState.columnWidths, DEFAULT_COL_W);
  }

  function toggleSort(column: string): void {
    appState.sortState = nextSortState(appState.sortState, column);
    clearSelectedCell();
    applyGridSort();
    renderHead();
    viewportEl.scrollTop = 0;
    actions.renderRows();
  }

  function startColumnResize(ev: MouseEvent, colName: string, handle: HTMLElement): void {
    ev.preventDefault();
    ev.stopPropagation();
    const startX = ev.clientX;
    const startW = colWidth(colName);
    handle.classList.add("dragging");
    document.body.style.cursor = "col-resize";
    function onMove(e: MouseEvent) {
      appState.columnWidths[colName] = resizedGridColumnWidth(startW, e.clientX - startX);
      renderHead();
      actions.renderRows();
    }
    function onUp() {
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
      document.body.style.cursor = "";
    }
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  }

  async function persistColumnOrder(remainingOrder: string[]): Promise<void> {
    const fieldIndex = buildSourceFieldIndex(appState.rows);
    const targets = appState.sources.filter((s) => (fieldIndex.get(s.id)?.size ?? 0) > 0);
    let done = 0;
    let failed = 0;
    for (const source of targets) {
      done += 1;
      actions.setResultInfo(`列の並び順を保存中… (${done}/${targets.length}) ${source.name}`);
      try {
        const detail = await invoke("get_source_detail", { id: source.id });
        const cfg = { ...detail.config } as Record<string, unknown>;
        const aliases = (cfg.column_aliases as Record<string, string> | undefined) ?? {};
        const sourceFields = toRawFieldNames(fieldIndex.get(source.id)!, aliases);
        const newOrder = sourceColumnOrderForDisplay(sourceFields, aliases, remainingOrder);
        const oldOrder = (cfg.column_order as string[] | undefined) ?? [];
        if (JSON.stringify(oldOrder) !== JSON.stringify(newOrder)) {
          cfg.column_order = newOrder;
          await invoke("update_source", {
            id: source.id,
            name: detail.name,
            configJson: JSON.stringify(cfg),
          });
        }
      } catch (err) {
        failed += 1;
        console.error(`列並び順更新失敗 (source ${source.id}):`, err);
      }
    }
    await actions.refreshSidebar();
    actions.setResultInfo(
      failed > 0
        ? `列の並び順を保存しました（${targets.length - failed}/${targets.length} 件成功、${failed} 件失敗）`
        : `${appState.rows.length} 件`,
    );
  }

  function renderHead(): void {
    headEl.innerHTML = "";
    const inner = document.createElement("div");
    inner.className = "grid-head-inner";
    appState.columns.forEach((columnName) => {
      const cell = document.createElement("div");
      cell.className = "cell head";
      cell.style.width = colWidth(columnName) + "px";
      const label = document.createElement("span");
      label.className = "cell-head-label";
      label.textContent = columnName;
      cell.appendChild(label);

      cell.classList.add("sortable");
      cell.title = gridSortHint(columnName, appState.sortState);
      if (appState.sortState?.column === columnName) {
        const mark = document.createElement("span");
        mark.className = "sort-mark";
        mark.textContent = appState.sortState.dir === "asc" ? "▲" : "▼";
        cell.appendChild(mark);
      }
      cell.addEventListener("click", () => {
        if (suppressHeadClick) return;
        toggleSort(columnName);
      });

      if (!PINNED_GRID_COLUMNS.has(columnName)) {
        cell.addEventListener("contextmenu", (ev) => {
          ev.preventDefault();
          void actions.openColumnSettings(columnName);
        });
        cell.classList.add("draggable-col");
        cell.draggable = true;
        cell.addEventListener("dragstart", (ev) => {
          gridDragSrcCol = columnName;
          suppressHeadClick = true;
          cell.classList.add("dragging");
          (ev as DragEvent).dataTransfer?.setData("text/plain", columnName);
        });
        cell.addEventListener("dragend", () => {
          cell.classList.remove("dragging");
          actions.clearDropTargets();
          setTimeout(() => { suppressHeadClick = false; }, 0);
        });
        cell.addEventListener("dragover", (ev) => {
          handleColumnDragOver(columnDropHighlight, ev as DragEvent, cell);
        });
        cell.addEventListener("drop", async (ev) => {
          ev.preventDefault();
          ev.stopPropagation();
          const rect = cell.getBoundingClientRect();
          const isAfter = (ev as MouseEvent).clientX >= rect.left + rect.width / 2;
          actions.clearDropTargets();
          if (gridDragSrcCol === null || gridDragSrcCol === columnName) return;
          const remainingOrder = reorderDisplayColumns(
            appState.columns,
            PINNED_GRID_COLUMNS,
            gridDragSrcCol,
            columnName,
            isAfter,
          );
          if (!remainingOrder) return;
          appState.gridColumnOrder = remainingOrder;
          rebuildGridColumns();
          renderHead();
          actions.renderRows();
          await persistColumnOrder(remainingOrder);
        });
      }

      const handle = document.createElement("div");
      handle.className = "col-resize-handle";
      handle.addEventListener("mousedown", (ev) => startColumnResize(ev as MouseEvent, columnName, handle));
      handle.addEventListener("click", (ev) => ev.stopPropagation());
      handle.addEventListener("contextmenu", (ev) => {
        ev.preventDefault();
        ev.stopPropagation();
      });
      cell.appendChild(handle);
      inner.appendChild(cell);
    });
    inner.style.width = actions.totalGridWidth() + "px";
    headEl.appendChild(inner);
  }

  headEl.addEventListener("dragleave", (ev) => {
    if (!headEl.contains(ev.relatedTarget as Node)) {
      clearColumnDropHighlight(columnDropHighlight);
    }
  });

  return { renderHead };
}

import { $ } from "../../app/dom";
import { invoke } from "../../app/ipc";
import { appState } from "../../app/state";
import { writeClipboardText } from "../../shared/clipboard";
import { showToast } from "../../app/notifications";
import type { Row } from "../../shared/types";
import { formatGridCellText, rowAsJson, rowAsTsv, rowAsTsvWithHeader } from "./cell";
import { gridColumnWidth } from "./data";
import {
  cellSelectionSuffix,
  isArrowKey,
  initialCellPosition,
  moveCellSelection,
} from "./selection";
import { scrollTopToRevealRow, visibleRowRange } from "./viewport";

const ROW_H = 26;
const DEFAULT_COL_W = 200;

export interface GridViewElements {
  viewportEl: HTMLDivElement;
  spacerEl: HTMLDivElement;
  rowsEl: HTMLDivElement;
  headEl: HTMLDivElement;
  gridRoot: HTMLDivElement;
}

export interface GridViewActions {
  closeAllContextMenus: () => void;
  openGridContextMenu: (ev: MouseEvent) => void;
  totalGridWidth: () => number;
  onRowsRendered: () => void;
}

export interface GridViewController {
  cellText: (row: Row, col: number, rowIndex?: number) => string;
  renderRows: () => void;
  selectCell: (row: number, col: number) => void;
  ensureRowVisible: (row: number) => void;
  copySelected: () => Promise<void>;
  copyRowAsTsv: () => Promise<void>;
  copyRowAsTsvWithHeader: () => Promise<void>;
  copyRowAsJson: () => Promise<void>;
  openSelected: () => Promise<void>;
}

function colWidth(name: string): number {
  return gridColumnWidth(name, appState.columnWidths, DEFAULT_COL_W);
}

async function copyToClipboard(text: string, label: string): Promise<void> {
  const result = await writeClipboardText(text, (value) => navigator.clipboard.writeText(value));
  showToast(result.ok ? `${label}しました` : `${label}に失敗しました: ${String(result.error)}`);
}

export function mountGridView(
  elements: GridViewElements,
  actions: GridViewActions,
): GridViewController {
  const { viewportEl, spacerEl, rowsEl, headEl, gridRoot } = elements;

  function cellText(row: Row, col: number, rowIndex?: number): string {
    return formatGridCellText(row, appState.columns[col], rowIndex);
  }

  function renderRows(): void {
    spacerEl.style.height = appState.rows.length * ROW_H + "px";
    const scrollTop = viewportEl.scrollTop;
    const viewH = viewportEl.clientHeight;
    const { start, end } = visibleRowRange(appState.rows.length, scrollTop, viewH, ROW_H);

    rowsEl.style.transform = `translateY(${start * ROW_H}px)`;
    rowsEl.innerHTML = "";
    const totalW = actions.totalGridWidth();
    for (let i = start; i < end; i++) {
      const row = appState.rows[i];
      const rowEl = document.createElement("div");
      rowEl.className = "grid-row" + (i % 2 ? " alt" : "");
      rowEl.style.height = ROW_H + "px";
      rowEl.style.width = totalW + "px";
      for (let c = 0; c < appState.columns.length; c++) {
        const cell = document.createElement("div");
        cell.className = "cell" + cellSelectionSuffix(appState.selectedCell, i, c);
        if (appState.columns[c] === "#") cell.classList.add("sys-seq");
        if (appState.columns[c] === "元行") cell.classList.add("sys-row-num");
        cell.style.width = colWidth(appState.columns[c]) + "px";
        cell.textContent = cellText(row, c, i);
        cell.onmousedown = () => selectCell(i, c);
        cell.addEventListener("contextmenu", (ev) => {
          ev.preventDefault();
          selectCell(i, c);
          actions.openGridContextMenu(ev as MouseEvent);
        });
        rowEl.appendChild(cell);
      }
      rowsEl.appendChild(rowEl);
    }
    actions.onRowsRendered();
  }

  function selectCell(row: number, col: number): void {
    actions.closeAllContextMenus();
    appState.selectedCell = { row, col };
    renderRows();
    gridRoot.focus();
  }

  function ensureRowVisible(row: number): void {
    viewportEl.scrollTop = scrollTopToRevealRow(row, viewportEl.scrollTop, viewportEl.clientHeight, ROW_H);
  }

  async function copySelected(): Promise<void> {
    if (!appState.selectedCell) return;
    await copyToClipboard(
      cellText(appState.rows[appState.selectedCell.row], appState.selectedCell.col),
      "コピー",
    );
  }

  async function copyRowAsTsv(): Promise<void> {
    if (!appState.selectedCell) return;
    const row = appState.rows[appState.selectedCell.row];
    await copyToClipboard(rowAsTsv(row, appState.columns, appState.selectedCell.row), "TSVコピー");
  }

  async function copyRowAsTsvWithHeader(): Promise<void> {
    if (!appState.selectedCell) return;
    const row = appState.rows[appState.selectedCell.row];
    await copyToClipboard(
      rowAsTsvWithHeader(row, appState.columns, appState.selectedCell.row),
      "見出し付きTSVコピー",
    );
  }

  async function copyRowAsJson(): Promise<void> {
    if (!appState.selectedCell) return;
    const row = appState.rows[appState.selectedCell.row];
    await copyToClipboard(rowAsJson(row, appState.columns, appState.selectedCell.row), "JSONコピー");
  }

  async function openSelected(): Promise<void> {
    if (!appState.selectedCell) return;
    const row = appState.rows[appState.selectedCell.row];
    if (!row.open_ref) return;
    try {
      await invoke("open_ref", { target: row.open_ref });
    } catch (e) {
      $("result-info").textContent = "開けませんでした: " + String(e);
    }
  }

  viewportEl.addEventListener("scroll", () => {
    headEl.scrollLeft = viewportEl.scrollLeft;
    renderRows();
  });
  window.addEventListener("resize", () => renderRows());

  gridRoot.addEventListener("keydown", (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "c") {
      e.preventDefault();
      void copySelected();
      return;
    }
    if (appState.rows.length === 0) return;
    if (!appState.selectedCell && isArrowKey(e.key)) {
      const start = initialCellPosition();
      selectCell(start.row, start.col);
      e.preventDefault();
      return;
    }
    if (!appState.selectedCell) return;
    if (!isArrowKey(e.key)) {
      if (e.key === "Enter") {
        e.preventDefault();
        void openSelected();
      }
      return;
    }
    e.preventDefault();
    const next = moveCellSelection(
      appState.selectedCell,
      e.key,
      appState.rows.length,
      appState.columns.length,
    );
    appState.selectedCell = next;
    ensureRowVisible(next.row);
    renderRows();
  });

  return {
    cellText,
    renderRows,
    selectCell,
    ensureRowVisible,
    copySelected,
    copyRowAsTsv,
    copyRowAsTsvWithHeader,
    copyRowAsJson,
    openSelected,
  };
}

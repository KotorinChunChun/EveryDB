import type { RoleState } from "./roles";
import { applyColumnOrder, collectRoleConfigFromTable } from "./roles";

export interface RoleEditorElements {
  rowsEl: HTMLTableSectionElement;
  tableWrap: HTMLDivElement;
  hintEl: HTMLDivElement;
}

export interface RoleEditorActions {
  clearDropTargets: () => void;
  handleVerticalDragOver: (ev: DragEvent, el: HTMLElement) => boolean;
}

export interface RoleEditorController {
  getRoleState: () => RoleState | null;
  setRoleState: (state: RoleState | null) => void;
  getLastFieldNames: () => readonly string[];
  render: (names: string[]) => void;
  reset: () => void;
  collectConfig: () => Record<string, unknown>;
  setHint: (text: string) => void;
  appendHint: (suffix: string) => void;
}

export function mountRoleEditor(
  elements: RoleEditorElements,
  actions: RoleEditorActions,
): RoleEditorController {
  const { rowsEl, tableWrap, hintEl } = elements;
  let roleState: RoleState | null = null;
  let lastFieldNames: string[] = [];
  let dragSrcName: string | null = null;

  function render(names: string[]): void {
    lastFieldNames = names;
    rowsEl.innerHTML = "";
    const orderedNames = applyColumnOrder(names, roleState?.order);
    for (const name of orderedNames) {
      const row = document.createElement("tr");
      row.dataset.name = name;
      row.draggable = true;
      row.addEventListener("dragstart", () => {
        dragSrcName = name;
        row.classList.add("dragging");
      });
      row.addEventListener("dragend", () => {
        row.classList.remove("dragging");
        actions.clearDropTargets();
      });
      row.addEventListener("dragover", (ev) => {
        actions.handleVerticalDragOver(ev as DragEvent, row);
      });
      row.addEventListener("drop", (ev) => {
        ev.preventDefault();
        ev.stopPropagation();
        const rect = row.getBoundingClientRect();
        const isAfter = (ev as MouseEvent).clientY >= rect.top + rect.height / 2;
        actions.clearDropTargets();
        if (dragSrcName === null || dragSrcName === name) return;
        const srcRow = rowsEl.querySelector(`tr[data-name="${CSS.escape(dragSrcName)}"]`);
        if (srcRow) {
          if (isAfter) rowsEl.insertBefore(srcRow, row.nextSibling);
          else rowsEl.insertBefore(srcRow, row);
        }
      });

      const handleTd = document.createElement("td");
      handleTd.className = "r-drag-handle";
      handleTd.textContent = "⠿";
      handleTd.title = "ドラッグで並べ替え";

      const nameTd = document.createElement("td");
      nameTd.textContent = name;
      nameTd.title = name;

      const dispTd = document.createElement("td");
      const dispCb = document.createElement("input");
      dispCb.type = "checkbox";
      dispCb.className = "r-display";
      dispCb.checked = roleState ? roleState.display.has(name) : true;
      dispTd.appendChild(dispCb);

      const searchTd = document.createElement("td");
      const searchCb = document.createElement("input");
      searchCb.type = "checkbox";
      searchCb.className = "r-search";
      searchCb.checked = roleState ? roleState.searchable.has(name) : true;
      searchTd.appendChild(searchCb);

      const labelTd = document.createElement("td");
      const labelRadio = document.createElement("input");
      labelRadio.type = "radio";
      labelRadio.name = "r-label";
      labelRadio.value = name;
      labelRadio.checked = roleState ? roleState.label_column === name : name === orderedNames[0];
      labelTd.appendChild(labelRadio);

      const openRefTd = document.createElement("td");
      const openRefRadio = document.createElement("input");
      openRefRadio.type = "radio";
      openRefRadio.name = "r-openref";
      openRefRadio.value = name;
      openRefRadio.checked = roleState ? roleState.open_ref_column === name : false;
      openRefTd.appendChild(openRefRadio);

      const aliasTd = document.createElement("td");
      const aliasInput = document.createElement("input");
      aliasInput.type = "text";
      aliasInput.className = "r-alias";
      aliasInput.placeholder = name;
      aliasInput.value = roleState?.aliases[name] ?? "";
      aliasTd.appendChild(aliasInput);

      row.append(handleTd, nameTd, dispTd, searchTd, labelTd, openRefTd, aliasTd);
      rowsEl.appendChild(row);
    }
    tableWrap.hidden = names.length === 0;
    hintEl.textContent = names.length ? `${names.length} 列を検出` : "フィールドが見つかりませんでした";
  }

  function reset(): void {
    tableWrap.hidden = true;
    rowsEl.innerHTML = "";
    lastFieldNames = [];
    hintEl.textContent = "接続情報を入力してから取得してください";
  }

  rowsEl.addEventListener("dragleave", (ev) => {
    if (!rowsEl.contains(ev.relatedTarget as Node)) {
      actions.clearDropTargets();
    }
  });

  return {
    getRoleState: () => roleState,
    setRoleState: (state) => { roleState = state; },
    getLastFieldNames: () => lastFieldNames,
    render,
    reset,
    collectConfig: () => collectRoleConfigFromTable(rowsEl, lastFieldNames),
    setHint: (text) => { hintEl.textContent = text; },
    appendHint: (suffix) => { hintEl.textContent += suffix; },
  };
}

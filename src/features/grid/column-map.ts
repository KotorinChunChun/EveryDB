import { $ } from "../../app/dom";
import { invoke } from "../../app/ipc";
import { appState } from "../../app/state";
import { withFieldVisibility } from "../source-editor/config-rules";
import type { SourceDetail } from "../../shared/types";
import {
  applyColumnAliasChange,
  columnMapVisibleDefault,
} from "./column-order";
import { buildSourceFieldIndex, toRawFieldNames } from "./data";

export interface ColumnMapElements {
  dialog: HTMLDialogElement;
  titleEl: HTMLDivElement;
  displayNameInput: HTMLInputElement;
  visibleCheckbox: HTMLInputElement;
  rowsBody: HTMLTableSectionElement;
  hintEl: HTMLDivElement;
  cancelBtn: HTMLButtonElement;
  closeBtn: HTMLButtonElement;
  saveBtn: HTMLButtonElement;
}

export interface ColumnMapActions {
  loadSources: () => Promise<void>;
  runSearch: () => Promise<void>;
}

export interface ColumnSourceMapRow {
  sourceId: number;
  sourceName: string;
  detail: SourceDetail;
  fields: string[];
  currentField: string | null;
}

export interface ColumnMapController {
  openColumnSettings: (columnName: string) => Promise<void>;
}

export function mountColumnMapDialog(
  elements: ColumnMapElements,
  actions: ColumnMapActions,
): ColumnMapController {
  let columnMapRows: ColumnSourceMapRow[] = [];
  let columnMapColumnName = "";

  function renderColumnMapRows(): void {
    elements.rowsBody.innerHTML = "";
    for (const row of columnMapRows) {
      const tr = document.createElement("tr");
      const nameTd = document.createElement("td");
      nameTd.textContent = row.sourceName;

      const selTd = document.createElement("td");
      const sel = document.createElement("select");
      const noneOpt = document.createElement("option");
      noneOpt.value = "";
      noneOpt.textContent = "（この列に対応しない）";
      sel.appendChild(noneOpt);
      for (const field of row.fields) {
        const opt = document.createElement("option");
        opt.value = field;
        opt.textContent = field;
        sel.appendChild(opt);
      }
      sel.value = row.currentField ?? "";
      selTd.appendChild(sel);

      tr.append(nameTd, selTd);
      elements.rowsBody.appendChild(tr);
    }
  }

  async function openColumnSettings(columnName: string): Promise<void> {
    columnMapColumnName = columnName;
    elements.titleEl.textContent = `列「${columnName}」の設定`;
    elements.displayNameInput.value = columnName;
    elements.visibleCheckbox.checked = true;
    elements.rowsBody.innerHTML = "";
    elements.hintEl.textContent = "";
    elements.dialog.showModal();

    const fieldIndex = buildSourceFieldIndex(appState.rows);
    const sourceIds = [...fieldIndex.keys()];
    elements.hintEl.textContent = "読み込み中…";
    const details = await Promise.all(
      sourceIds.map((sid) =>
        invoke("get_source_detail", { id: sid }).catch((e) => {
          console.error(`列マッピング取得失敗 (source ${sid}):`, e);
          return null;
        }),
      ),
    );
    columnMapRows = [];
    for (let i = 0; i < sourceIds.length; i++) {
      const sid = sourceIds[i];
      const detail = details[i];
      if (!detail) continue;
      const aliases = (detail.config.column_aliases as Record<string, string> | undefined) ?? {};
      const fields = toRawFieldNames(fieldIndex.get(sid)!, aliases);
      let currentField: string | null = null;
      for (const field of fields) {
        if ((aliases[field] ?? field) === columnName) {
          currentField = field;
          break;
        }
      }
      const src = appState.sources.find((s) => s.id === sid);
      columnMapRows.push({
        sourceId: sid,
        sourceName: src?.name ?? `#${sid}`,
        detail,
        fields,
        currentField,
      });
    }

    const visibleDefault = columnMapVisibleDefault(
      columnMapRows.map((row) => ({
        currentField: row.currentField,
        displayColumns: row.detail.config.display_columns as string[] | undefined,
      })),
    );
    if (visibleDefault !== null) elements.visibleCheckbox.checked = visibleDefault;

    renderColumnMapRows();
    elements.hintEl.textContent = columnMapRows.length ? "" : "対応するデータソースが見つかりませんでした";
  }

  elements.cancelBtn.onclick = () => elements.dialog.close();
  elements.closeBtn.onclick = () => elements.dialog.close();

  elements.saveBtn.onclick = async () => {
    const newName = elements.displayNameInput.value.trim() || columnMapColumnName;
    const visible = elements.visibleCheckbox.checked;
    const selects = Array.from(elements.rowsBody.querySelectorAll("select"));
    try {
      for (let i = 0; i < columnMapRows.length; i++) {
        const row = columnMapRows[i];
        const newField = selects[i].value || null;
        if (newField === null && row.currentField === null) continue;

        const cfg = { ...row.detail.config } as Record<string, unknown>;
        cfg.column_aliases = applyColumnAliasChange(
          (cfg.column_aliases as Record<string, string> | undefined) ?? {},
          row.currentField,
          columnMapColumnName,
          newField,
          newName,
        );

        if (newField) {
          const displayColumns = withFieldVisibility(
            cfg.display_columns as string[] | undefined,
            row.fields,
            newField,
            visible,
          );
          if (displayColumns !== undefined) cfg.display_columns = displayColumns;
        }

        await invoke("update_source", {
          id: row.sourceId,
          name: row.detail.name,
          configJson: JSON.stringify(cfg),
        });
      }
      elements.dialog.close();
      await actions.loadSources();
      await actions.runSearch();
    } catch (e) {
      alert("保存に失敗: " + String(e));
    }
  };

  return { openColumnSettings };
}

export function mountColumnMapDialogFromDom(actions: ColumnMapActions): ColumnMapController {
  return mountColumnMapDialog({
    dialog: $<HTMLDialogElement>("column-map-dialog"),
    titleEl: $<HTMLDivElement>("column-map-title"),
    displayNameInput: $<HTMLInputElement>("cm-display-name"),
    visibleCheckbox: $<HTMLInputElement>("cm-visible"),
    rowsBody: $<HTMLTableSectionElement>("column-map-rows"),
    hintEl: $<HTMLDivElement>("column-map-hint"),
    cancelBtn: $<HTMLButtonElement>("column-map-cancel"),
    closeBtn: $<HTMLButtonElement>("column-map-close-x"),
    saveBtn: $<HTMLButtonElement>("column-map-save"),
  }, actions);
}

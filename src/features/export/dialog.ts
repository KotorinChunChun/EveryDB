import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { writeFile } from "@tauri-apps/plugin-fs";
import { $ } from "../../app/dom";
import { invoke } from "../../app/ipc";
import type { Row } from "../../shared/types";
import {
  buildExportText,
  encodeExportText,
  EXPORT_FORMAT_INFO,
  type ExportEncoding,
  type ExportFormat,
} from "./serialization";

export interface ExportDialogDeps {
  getRowCount: () => number;
  getColumnCount: () => number;
  getColumns: () => readonly string[];
  getRows: () => readonly Row[];
  cellText: (row: Row, col: number, rowIndex?: number) => string;
  setResultInfo: (text: string) => void;
}

export interface ExportDialogController {
  openExportDialog: () => void;
}

function currentExportFormat(): ExportFormat {
  return (document.querySelector<HTMLInputElement>('input[name="export-format"]:checked')?.value as ExportFormat) ?? "tsv";
}

/** 検索結果エクスポートダイアログをマウントする。 */
export function mountExportDialog(deps: ExportDialogDeps): ExportDialogController {
  const exportEncodingSel = $<HTMLSelectElement>("export-encoding");
  const exportEncodingHint = $<HTMLDivElement>("export-encoding-hint");

  function updateExportEncodingState() {
    const isJson = currentExportFormat() === "json";
    exportEncodingSel.disabled = isJson;
    exportEncodingHint.textContent = isJson ? "JSON は UTF-8（BOMなし）固定です" : "";
    if (isJson) exportEncodingSel.value = "utf8";
  }

  for (const el of document.querySelectorAll<HTMLInputElement>('input[name="export-format"]')) {
    el.addEventListener("change", updateExportEncodingState);
  }

  function openExportDialog() {
    if (deps.getRowCount() === 0) {
      deps.setResultInfo("エクスポートする結果がありません");
      return;
    }
    $<HTMLDivElement>("export-error").textContent = "";
    $<HTMLDivElement>("export-count").textContent = `${deps.getRowCount()} 件 / ${deps.getColumnCount()} 列を出力します`;
    $<HTMLInputElement>("export-format-tsv").checked = true;
    exportEncodingSel.value = "utf8bom";
    updateExportEncodingState();
    $<HTMLDialogElement>("export-dialog").showModal();
  }

  $<HTMLButtonElement>("export").onclick = () => openExportDialog();
  $<HTMLButtonElement>("export-cancel").onclick = () => $<HTMLDialogElement>("export-dialog").close();
  $<HTMLButtonElement>("export-close-x").onclick = () => $<HTMLDialogElement>("export-dialog").close();

  $<HTMLButtonElement>("export-ok").onclick = async () => {
    const format = currentExportFormat();
    const encoding = exportEncodingSel.value as ExportEncoding;
    const info = EXPORT_FORMAT_INFO[format];
    const errEl = $<HTMLDivElement>("export-error");
    const statusEl = $<HTMLSpanElement>("export-status");
    const okBtn = $<HTMLButtonElement>("export-ok");
    const cancelBtn = $<HTMLButtonElement>("export-cancel");
    errEl.textContent = "";
    try {
      const path = await saveDialog({
        defaultPath: `everydb_export.${info.ext}`,
        filters: [{ name: info.filterName, extensions: [info.ext, ...info.extra] }],
      });
      if (!path) return;
      okBtn.disabled = true;
      cancelBtn.disabled = true;
      statusEl.hidden = false;
      const columns = deps.getColumns();
      const rows = deps.getRows();
      const text = buildExportText(format, columns, rows, deps.cellText);
      const bytes = await encodeExportText(
        text,
        format === "json" ? "utf8" : encoding,
        async (value) => new Uint8Array(await invoke<number[]>("encode_shift_jis", { text: value })),
      );
      await writeFile(path, bytes);
      $<HTMLDialogElement>("export-dialog").close();
      deps.setResultInfo(`エクスポート完了: ${path}（${rows.length} 件）`);
    } catch (e) {
      errEl.textContent = "エクスポート失敗: " + String(e);
    } finally {
      okBtn.disabled = false;
      cancelBtn.disabled = false;
      statusEl.hidden = true;
    }
  };

  return { openExportDialog };
}

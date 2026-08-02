import { $, setPickerMessage } from "../../app/dom";
import { invoke } from "../../app/ipc";
import { appState } from "../../app/state";
import { colLetterOf, formatSelectedColumns, parseColumnSelection } from "../../shared/excel-columns";
import { SECRET_PLACEHOLDER } from "../source-editor/constants";
import type { SourcePickerDeps, SourcePickers } from "./types";

export type { SourcePickerDeps, SourcePickers } from "./types";

interface AirtableBaseItem {
  id: string;
  name: string;
  permission_level: string;
}
interface AirtableViewItem {
  id: string;
  name: string;
  type_: string;
}
interface AirtableTableItem {
  id: string;
  name: string;
  primary_field_id: string;
  views: AirtableViewItem[];
}

export function mountSourcePickers(deps: SourcePickerDeps): SourcePickers {
  const { getSourceEditor, getParentSettings, normalizePath, loadSources } = deps;

  function getTargetExcelPath(): string {
    const pathEl = (document.getElementById("fd-path") || document.getElementById("pf-path")) as HTMLInputElement | null;
    const sourceEditor = getSourceEditor();
    const editingId = sourceEditor.getEditingId();
    const hiddenSharedValues = sourceEditor.getHiddenSharedValues();
    const raw = pathEl?.value || String(hiddenSharedValues["path"] ?? "") || (editingId !== null ? appState.sources.find((s) => s.id === editingId)?.parent : "") || "";
    return normalizePath(raw);
  }

  async function openNotionPicker() {
    const tokenEl = document.getElementById("fd-token") as HTMLInputElement | null;
    let token = tokenEl?.value.trim();
    if (tokenEl?.dataset.credRef) {
      try {
        token = await invoke<string>("credential_secret_value", { id: Number(tokenEl.dataset.credRef) });
      } catch (e) {
        alert("トークンの取得に失敗しました: " + String(e));
        return;
      }
    }
    if (!token || token === SECRET_PLACEHOLDER) { alert("先にトークンを選択してください"); return; }
    const listEl = $<HTMLDivElement>("notion-picker-list");
    listEl.innerHTML = "<div class='picker-empty'>読み込み中…</div>";
    const dlg = $<HTMLDialogElement>("notion-picker");
    dlg.showModal();
    try {
      const dbs = await invoke<{ id: string; title: string }[]>("browse_notion_databases", { token });
      listEl.innerHTML = "";
      if (dbs.length === 0) {
        listEl.innerHTML = "<div class='picker-empty'>アクセス可能なデータベースがありません（インテグレーションに共有してください）</div>";
      }
      for (const db of dbs) {
        const item = document.createElement("div");
        item.className = "picker-item";
        item.textContent = db.title || "(無題)";
        item.onclick = () => {
          (document.getElementById("fd-database_id") as HTMLInputElement).value = db.id;
          const sourceEditor = getSourceEditor();
          sourceEditor.autoFillDefaultName();
          sourceEditor.maybeFetchAndRenderFields();
          dlg.close();
        };
        listEl.appendChild(item);
      }
    } catch (e) {
      setPickerMessage(listEl, `取得失敗: ${String(e)}`);
    }
  }

  async function openBrowserProfilePicker(target: "history" | "bookmarks", pathInput: HTMLInputElement) {
    const browserSel = document.getElementById("fd-browser") as HTMLSelectElement | null;
    const browser = browserSel?.value ?? "chrome";
    const listEl = $<HTMLDivElement>("browser-profile-picker-list");
    listEl.innerHTML = "<div class='picker-empty'>読み込み中…</div>";
    const dlg = $<HTMLDialogElement>("browser-profile-picker");
    dlg.showModal();
    try {
      const entries = await invoke<{ label: string; path: string }[]>("browse_browser_profiles", { browser, target });
      listEl.innerHTML = "";
      if (entries.length === 0) {
        listEl.innerHTML = "<div class='picker-empty'>候補が見つかりませんでした</div>";
      }
      for (const entry of entries) {
        const item = document.createElement("div");
        item.className = "picker-item";
        item.textContent = entry.label;
        item.title = entry.path;
        item.onclick = () => {
          pathInput.value = entry.path;
          const sourceEditor = getSourceEditor();
          sourceEditor.maybeAutoFillName(entry.path);
          sourceEditor.maybeFetchAndRenderFields();
          dlg.close();
        };
        listEl.appendChild(item);
      }
    } catch (e) {
      setPickerMessage(listEl, `取得失敗: ${String(e)}`);
    }
  }

  async function openSheetPicker(sheetInput: HTMLInputElement) {
    const path = getTargetExcelPath();
    if (!path) {
      alert("先にファイルパスを入力・選択してください");
      return;
    }
    const titleEl = $<HTMLHeadingElement>("sheet-picker-title");
    if (titleEl) titleEl.textContent = "シートを選択";
    const listEl = $<HTMLDivElement>("sheet-picker-list");
    listEl.innerHTML = "<div class='picker-empty'>読み込み中…</div>";
    const dlg = $<HTMLDialogElement>("sheet-picker");
    dlg.showModal();
    try {
      const sheets = await invoke<string[]>("browse_excel_sheets", { path });
      listEl.innerHTML = "";
      if (sheets.length === 0) {
        listEl.innerHTML = "<div class='picker-empty'>シートが見つかりませんでした</div>";
      }
      for (const sheet of sheets) {
        const item = document.createElement("div");
        item.className = "picker-item";
        item.textContent = sheet;
        item.onclick = () => {
          sheetInput.value = sheet;
          const sourceEditor = getSourceEditor();
          sourceEditor.autoFillDefaultName();
          sourceEditor.maybeFetchAndRenderFields();
          dlg.close();
        };
        listEl.appendChild(item);
      }
    } catch (e) {
      setPickerMessage(listEl, `取得失敗: ${String(e)}`);
    }
  }

  async function openExcelTablePicker(tableInput: HTMLInputElement) {
    const path = getTargetExcelPath();
    if (!path) {
      alert("先にファイルパスを入力・選択してください");
      return;
    }
    const titleEl = $<HTMLHeadingElement>("sheet-picker-title");
    if (titleEl) titleEl.textContent = "テーブル(ListObject)を選択";
    const listEl = $<HTMLDivElement>("sheet-picker-list");
    listEl.innerHTML = "<div class='picker-empty'>読み込み中…</div>";
    const dlg = $<HTMLDialogElement>("sheet-picker");
    dlg.showModal();
    try {
      const tables = await invoke<string[]>("browse_excel_tables", { path });
      listEl.innerHTML = "";
      if (tables.length === 0) {
        listEl.innerHTML = "<div class='picker-empty'>テーブルが見つかりませんでした</div>";
      }
      for (const tbl of tables) {
        const item = document.createElement("div");
        item.className = "picker-item";
        item.textContent = tbl;
        item.onclick = () => {
          tableInput.value = tbl;
          const sourceEditor = getSourceEditor();
          sourceEditor.autoFillDefaultName();
          sourceEditor.maybeFetchAndRenderFields();
          dlg.close();
        };
        listEl.appendChild(item);
      }
    } catch (e) {
      setPickerMessage(listEl, `取得失敗: ${String(e)}`);
    }
  }

  async function fetchAirtableBasesForPicker(): Promise<AirtableBaseItem[]> {
    const tokenEl = (document.getElementById("fd-token") ?? document.getElementById("pf-token")) as HTMLInputElement | null;
    if (tokenEl?.dataset.credRef) {
      const token = await invoke<string>("credential_secret_value", { id: Number(tokenEl.dataset.credRef) });
      return await invoke<AirtableBaseItem[]>("browse_airtable_bases", { token });
    }
    const sourceEditor = getSourceEditor();
    const parentSettings = getParentSettings();
    const sid = sourceEditor.getEditingId() ?? parentSettings.getSiblingSourceId();
    if (sid) {
      const overrides = sourceEditor.getEditingId() !== null
        ? JSON.stringify(sourceEditor.getConfigFromFields().config)
        : JSON.stringify(parentSettings.getParentSharedValues());
      return await invoke<AirtableBaseItem[]>("browse_airtable_bases_for_source", { id: sid, overridesJson: overrides });
    }
    alert("先にトークンを選択してください");
    throw new Error("トークン未選択");
  }

  async function fetchAirtableTablesForPicker(baseIdInput?: string): Promise<AirtableTableItem[]> {
    const tokenEl = (document.getElementById("fd-token") ?? document.getElementById("pf-token")) as HTMLInputElement | null;
    const baseEl = (document.getElementById("fd-base_id") ?? document.getElementById("pf-base_id")) as HTMLInputElement | null;
    const baseId = baseIdInput ?? baseEl?.value.trim() ?? "";
    if (!baseId) {
      alert("先に Base ID を入力・選択してください");
      throw new Error("Base ID 必須");
    }
    if (tokenEl?.dataset.credRef) {
      const token = await invoke<string>("credential_secret_value", { id: Number(tokenEl.dataset.credRef) });
      return await invoke<AirtableTableItem[]>("browse_airtable_tables", { token, baseId });
    }
    const sourceEditor = getSourceEditor();
    const parentSettings = getParentSettings();
    const sid = sourceEditor.getEditingId() ?? parentSettings.getSiblingSourceId();
    if (sid) {
      const overrides = sourceEditor.getEditingId() !== null
        ? JSON.stringify(sourceEditor.getConfigFromFields().config)
        : JSON.stringify(parentSettings.getParentSharedValues());
      return await invoke<AirtableTableItem[]>("browse_airtable_tables_for_source", { id: sid, overridesJson: overrides, baseId: baseId || null });
    }
    alert("先にトークンを選択してください");
    throw new Error("トークン未選択");
  }

  async function openAirtableBasePicker(baseInput: HTMLInputElement) {
    const titleEl = $<HTMLHeadingElement>("airtable-picker-title");
    if (titleEl) titleEl.textContent = "Airtable Base を選択";
    const listEl = $<HTMLDivElement>("airtable-picker-list");
    listEl.innerHTML = "<div class='picker-empty'>読み込み中…</div>";
    const dlg = $<HTMLDialogElement>("airtable-picker");
    dlg.showModal();
    try {
      const bases = await fetchAirtableBasesForPicker();
      listEl.innerHTML = "";
      if (bases.length === 0) {
        listEl.innerHTML = "<div class='picker-empty'>利用可能な Base が見つかりませんでした（権限や schema.bases:read スコープを確認してください）</div>";
      }
      for (const b of bases) {
        const item = document.createElement("div");
        item.className = "picker-item";
        item.textContent = b.name ? `${b.name} [ID: ${b.id}]` : b.id;
        item.onclick = () => {
          baseInput.value = b.id;
          const nameEl = (document.getElementById("fd-base_name") ?? document.getElementById("pf-base_name")) as HTMLInputElement | null;
          if (nameEl) nameEl.value = b.name;
          const sourceEditor = getSourceEditor();
          sourceEditor.autoFillDefaultName();
          sourceEditor.maybeFetchAndRenderFields();
          dlg.close();
        };
        listEl.appendChild(item);
      }
    } catch (e) {
      setPickerMessage(listEl, `取得失敗: ${String(e)}`);
    }
  }

  async function openAirtableTablePicker(tableInput: HTMLInputElement) {
    const titleEl = $<HTMLHeadingElement>("airtable-picker-title");
    if (titleEl) titleEl.textContent = "Airtable テーブルを選択";
    const listEl = $<HTMLDivElement>("airtable-picker-list");
    listEl.innerHTML = "<div class='picker-empty'>読み込み中…</div>";
    const dlg = $<HTMLDialogElement>("airtable-picker");
    dlg.showModal();
    try {
      const tables = await fetchAirtableTablesForPicker();
      listEl.innerHTML = "";
      if (tables.length === 0) {
        listEl.innerHTML = "<div class='picker-empty'>テーブルが見つかりませんでした</div>";
      }
      for (const t of tables) {
        const item = document.createElement("div");
        item.className = "picker-item";
        item.textContent = t.name ? `${t.name} [ID: ${t.id}]` : t.id;
        item.onclick = () => {
          tableInput.value = t.id;
          const nameEl = (document.getElementById("fd-table_name") ?? document.getElementById("pf-table_name")) as HTMLInputElement | null;
          if (nameEl) nameEl.value = t.name;
          const sourceEditor = getSourceEditor();
          sourceEditor.autoFillDefaultName();
          sourceEditor.maybeFetchAndRenderFields();
          dlg.close();
        };
        listEl.appendChild(item);
      }
    } catch (e) {
      setPickerMessage(listEl, `取得失敗: ${String(e)}`);
    }
  }

  async function openAirtableViewPicker(viewInput: HTMLInputElement) {
    const tableEl = document.getElementById("fd-table") as HTMLInputElement | null;
    const tableVal = tableEl?.value.trim() ?? "";
    if (!tableVal) {
      alert("先にテーブルを指定・選択してください");
      return;
    }
    const titleEl = $<HTMLHeadingElement>("airtable-picker-title");
    if (titleEl) titleEl.textContent = "Airtable ビューを選択";
    const listEl = $<HTMLDivElement>("airtable-picker-list");
    listEl.innerHTML = "<div class='picker-empty'>読み込み中…</div>";
    const dlg = $<HTMLDialogElement>("airtable-picker");
    dlg.showModal();
    try {
      const tables = await fetchAirtableTablesForPicker();
      const table = tables.find(t => t.id === tableVal || t.name === tableVal || t.id.toLowerCase() === tableVal.toLowerCase() || t.name.toLowerCase() === tableVal.toLowerCase());
      listEl.innerHTML = "";
      if (!table || !table.views || table.views.length === 0) {
        listEl.innerHTML = "<div class='picker-empty'>このテーブルに選択可能なビューが見つかりませんでした</div>";
        return;
      }
      for (const v of table.views) {
        const item = document.createElement("div");
        item.className = "picker-item";
        item.textContent = v.name ? `${v.name} [ID: ${v.id} / 種別: ${v.type_}]` : v.id;
        item.onclick = () => {
          viewInput.value = v.id;
          const nameEl = (document.getElementById("fd-view_name") ?? document.getElementById("pf-view_name")) as HTMLInputElement | null;
          if (nameEl) nameEl.value = v.name;
          const sourceEditor = getSourceEditor();
          sourceEditor.autoFillDefaultName();
          sourceEditor.maybeFetchAndRenderFields();
          dlg.close();
        };
        listEl.appendChild(item);
      }
    } catch (e) {
      setPickerMessage(listEl, `取得失敗: ${String(e)}`);
    }
  }

  async function openCellPicker(cellInput: HTMLInputElement, gridMode: boolean | "top" | "bottom" | "bottom_left" | "bottom_right" = "top", pickMode: "cell" | "column" = "cell") {
    const path = getTargetExcelPath();
    if (!path) {
      alert("先にファイルパスを入力・選択してください");
      return;
    }
    const sourceEditor = getSourceEditor();
    const sheetEl = document.getElementById("fd-sheet") as HTMLInputElement | null;
    const sheet = (sheetEl?.value.trim() || String(sourceEditor.getHiddenSharedValues()["sheet"] ?? "") || "").trim();
    if (!sheet) {
      alert("先にシート名を入力・選択してください");
      return;
    }
    const wrapEl = $<HTMLDivElement>("cell-picker-grid-wrap");
    wrapEl.innerHTML = "<div class='picker-empty'>読み込み中…</div>";
    const titlePrefix = pickMode === "column" ? "基準列を選択" : "セルを選択";
    $<HTMLHeadingElement>("cell-picker-title").textContent = `${titlePrefix} (シート: ${sheet})`;
    const dlg = $<HTMLDialogElement>("cell-picker");
    const confirmBtn = $<HTMLButtonElement>("cell-picker-confirm");
    const statusEl = $<HTMLSpanElement>("cell-picker-status");
    if (pickMode === "column") {
      confirmBtn.style.display = "inline-block";
    } else {
      confirmBtn.style.display = "none";
    }
    dlg.showModal();

    const hintEl = document.getElementById("cell-picker-hint") as HTMLDivElement | null;
    if (hintEl) {
      hintEl.style.display = pickMode === "column" ? "block" : "none";
    }

    let isDragging = false;
    let dragStartCol = -1;
    const initialValue = cellInput.value.trim();
    const selectedCols = pickMode === "column" ? parseColumnSelection(initialValue) : new Set<number>();
    const initialCell = pickMode === "cell" ? initialValue.toUpperCase() : "";
    let dragBaseSelected = new Set<number>();

    const updateSelectionDisplay = (table: HTMLTableElement) => {
      if (pickMode !== "column") return;
      table.querySelectorAll("th[data-col], td[data-col]").forEach((el) => {
        const colIdx = Number((el as HTMLElement).dataset.col);
        if (selectedCols.has(colIdx)) el.classList.add("col-selected");
        else el.classList.remove("col-selected");
      });
      if (selectedCols.size > 0) {
        statusEl.textContent = "選択中の列: " + formatSelectedColumns(selectedCols);
      } else {
        statusEl.textContent = "列をクリックまたはドラッグで選択してください (Ctrlキーで複数選択)";
      }
    };

    try {
      const res = await invoke<{ start_row: number; start_col: number; rows: string[][] }>("browse_excel_grid", { path, sheet, rows: 10, cols: 10, mode: typeof gridMode === "string" ? gridMode : "top" });
      wrapEl.innerHTML = "";
      const table = document.createElement("table");
      table.className = "cell-grid";

      const thead = document.createElement("thead");
      const headerRow = document.createElement("tr");
      const cornerTh = document.createElement("th");
      cornerTh.className = "corner-header";
      headerRow.appendChild(cornerTh);
      for (let c = 0; c < 10; c++) {
        const th = document.createElement("th");
        const absCol = (res.start_col || 0) + c;
        th.textContent = colLetterOf(absCol);
        th.dataset.col = String(absCol);
        headerRow.appendChild(th);
      }
      thead.appendChild(headerRow);
      table.appendChild(thead);

      const tbody = document.createElement("tbody");
      const rowValsList = res.rows || [];
      for (let r = 0; r < 10; r++) {
        const tr = document.createElement("tr");
        const absRow = (res.start_row || 0) + r + 1;
        const rowTh = document.createElement("th");
        rowTh.className = "row-header";
        rowTh.textContent = String(absRow);
        tr.appendChild(rowTh);

        const rowVals = rowValsList[r] || [];
        for (let c = 0; c < 10; c++) {
          const td = document.createElement("td");
          const absCol = (res.start_col || 0) + c;
          const colLetter = colLetterOf(absCol);
          const addr = `${colLetter}${absRow}`;
          const val = rowVals[c] ?? "";
          td.dataset.col = String(absCol);
          if (pickMode === "cell" && initialCell && addr === initialCell) {
            td.classList.add("cell-selected");
          }

          const addrSpan = document.createElement("span");
          addrSpan.className = "cell-addr";
          addrSpan.textContent = addr;
          td.appendChild(addrSpan);

          const valSpan = document.createElement("span");
          valSpan.textContent = val !== "" ? val : "(空)";
          if (val === "") valSpan.style.color = "var(--muted)";
          td.appendChild(valSpan);

          td.title = `${addr}: ${val || "(空)"}`;
          if (pickMode === "cell") {
            td.onclick = () => {
              table.querySelectorAll("td.cell-selected").forEach((el) => el.classList.remove("cell-selected"));
              td.classList.add("cell-selected");
              cellInput.value = addr;
              const se = getSourceEditor();
              se.autoFillDefaultName();
              se.maybeFetchAndRenderFields();
              dlg.close();
            };
          }
          tr.appendChild(td);
        }
        tbody.appendChild(tr);
      }
      table.appendChild(tbody);
      wrapEl.appendChild(table);

      if (pickMode === "column") {
        updateSelectionDisplay(table);
        const onMouseDown = (ev: MouseEvent) => {
          const target = (ev.target as HTMLElement).closest("th[data-col], td[data-col]") as HTMLElement | null;
          if (!target || ev.button !== 0) return;
          ev.preventDefault();
          const c = Number(target.dataset.col);
          isDragging = true;
          dragStartCol = c;
          if (ev.ctrlKey || ev.metaKey) {
            dragBaseSelected = new Set(selectedCols);
            if (dragBaseSelected.has(c)) {
              selectedCols.delete(c);
            } else {
              selectedCols.add(c);
            }
          } else {
            dragBaseSelected = new Set();
            selectedCols.clear();
            selectedCols.add(c);
          }
          updateSelectionDisplay(table);
        };

        const onMouseOver = (ev: MouseEvent) => {
          if (!isDragging) return;
          const target = (ev.target as HTMLElement).closest("th[data-col], td[data-col]") as HTMLElement | null;
          if (!target) return;
          const c = Number(target.dataset.col);
          selectedCols.clear();
          for (const x of dragBaseSelected) selectedCols.add(x);
          const minC = Math.min(dragStartCol, c);
          const maxC = Math.max(dragStartCol, c);
          for (let x = minC; x <= maxC; x++) selectedCols.add(x);
          updateSelectionDisplay(table);
        };

        const onMouseUp = () => {
          isDragging = false;
        };

        table.addEventListener("mousedown", onMouseDown);
        table.addEventListener("mouseover", onMouseOver);
        document.addEventListener("mouseup", onMouseUp);

        const cleanup = () => {
          document.removeEventListener("mouseup", onMouseUp);
          confirmBtn.onclick = null;
          dlg.removeEventListener("close", cleanup);
        };
        dlg.addEventListener("close", cleanup);

        confirmBtn.onclick = () => {
          cellInput.value = formatSelectedColumns(selectedCols);
          const se = getSourceEditor();
          se.autoFillDefaultName();
          se.maybeFetchAndRenderFields();
          dlg.close();
        };
      } else {
        statusEl.textContent = "クリックでセルを選択してください";
      }
    } catch (e) {
      wrapEl.innerHTML = `<div class='picker-empty'>取得失敗: ${String(e)}</div>`;
    }
  }

  const tablePickerDlg = $<HTMLDialogElement>("table-picker");
  const tablePickerList = $<HTMLDivElement>("table-picker-list");
  const tablePickerCount = $<HTMLSpanElement>("table-picker-count");
  const tablePickerError = $<HTMLDivElement>("table-picker-error");

  function updateTablePickerCount() {
    const n = tablePickerList.querySelectorAll("input:checked").length;
    tablePickerCount.textContent = `${n} 件選択中`;
  }

  async function openTablePicker(browse: "sqlite" | "accdb" | "excel_table") {
    const path = getTargetExcelPath();
    if (!path) { alert("先にファイルパスを入力・選択してください"); return; }
    $<HTMLDivElement>("table-picker-title").textContent = browse === "excel_table" ? "テーブル(ListObject)一覧" : "テーブル一覧";
    tablePickerError.textContent = "";
    tablePickerList.innerHTML = "<div class='picker-empty'>読み込み中…</div>";
    tablePickerCount.textContent = "";
    tablePickerDlg.showModal();
    const cmd = browse === "sqlite" ? "browse_sqlite_tables" : browse === "accdb" ? "browse_accdb_tables" : "browse_excel_tables";
    let names: string[];
    try {
      names = await invoke<string[]>(cmd, { path });
    } catch (e) {
      setPickerMessage(tablePickerList, `取得失敗: ${String(e)}`);
      return;
    }
    tablePickerList.innerHTML = "";
    if (names.length === 0) {
      tablePickerList.innerHTML = "<div class='picker-empty'>見つかりませんでした</div>";
    }
    for (const name of names) {
      const item = document.createElement("label");
      item.className = "picker-item";
      const cb = document.createElement("input");
      cb.type = "checkbox";
      cb.value = name;
      cb.addEventListener("change", updateTablePickerCount);
      const span = document.createElement("span");
      span.textContent = name;
      item.append(cb, span);
      tablePickerList.appendChild(item);
    }
    updateTablePickerCount();

    $<HTMLButtonElement>("table-picker-add").onclick = async () => {
      const checked = Array.from(tablePickerList.querySelectorAll("input:checked")) as HTMLInputElement[];
      if (checked.length === 0) { tablePickerError.textContent = "1 つ以上チェックしてください"; return; }
      const type_ = browse === "excel_table" ? "excel_table" : browse;
      const failures: string[] = [];
      let added = 0;
      for (const cb of checked) {
        const tableName = cb.value;
        const config: Record<string, unknown> = { path, table: tableName };
        try {
          await invoke("add_source", { type: type_, name: tableName, configJson: JSON.stringify(config) });
          added++;
        } catch (e) {
          failures.push(`${tableName}: ${String(e)}`);
        }
      }
      await loadSources();
      if (failures.length) {
        tablePickerError.textContent = `${added} 件追加、${failures.length} 件失敗: ${failures[0]}`;
        return;
      }
      tablePickerDlg.close();
      $<HTMLDialogElement>("add-dialog").close();
    };
  }

  let spMode: "sites" | "teams" = "sites";

  async function graphAuthFromDialog(): Promise<{ tenant_id: string; client_id: string; client_secret: string } | null> {
    const tenant_id = (document.getElementById("fd-tenant_id") as HTMLInputElement | null)?.value.trim();
    const client_id = (document.getElementById("fd-client_id") as HTMLInputElement | null)?.value.trim();
    const secretEl = document.getElementById("fd-client_secret") as HTMLInputElement | null;
    if (!tenant_id || !client_id) return null;
    let client_secret: string | undefined;
    if (secretEl?.dataset.credRef) {
      client_secret = await invoke<string>("credential_secret_value", { id: Number(secretEl.dataset.credRef) });
    }
    if (!client_secret) return null;
    return { tenant_id, client_id, client_secret };
  }

  async function loadSpSourceList(auth: { tenant_id: string; client_id: string; client_secret: string }) {
    const listEl = $<HTMLDivElement>("sp-source-list");
    listEl.innerHTML = "<div class='picker-empty'>読み込み中…</div>";
    try {
      if (spMode === "sites") {
        const sites = await invoke<{ web_url: string; display_name: string }[]>("browse_graph_sites", { auth });
        renderSpSourceList(sites.map((s) => ({ key: s.web_url, label: s.display_name || s.web_url })), auth, "site");
      } else {
        const teams = await invoke<{ group_id: string; display_name: string }[]>("browse_graph_teams", { auth });
        renderSpSourceList(teams.map((t) => ({ key: t.group_id, label: t.display_name })), auth, "team");
      }
    } catch (e) {
      setPickerMessage(listEl, `取得失敗: ${String(e)}`);
    }
  }

  function renderSpSourceList(
    items: { key: string; label: string }[],
    auth: { tenant_id: string; client_id: string; client_secret: string },
    kind: "site" | "team",
  ) {
    const listEl = $<HTMLDivElement>("sp-source-list");
    const filterEl = $<HTMLInputElement>("sp-filter");
    const draw = () => {
      const q = filterEl.value.trim().toLowerCase();
      listEl.innerHTML = "";
      const filtered = items.filter((it) => it.label.toLowerCase().includes(q));
      if (filtered.length === 0) listEl.innerHTML = "<div class='picker-empty'>見つかりませんでした</div>";
      for (const it of filtered) {
        const row = document.createElement("div");
        row.className = "picker-item";
        row.textContent = it.label;
        row.onclick = async () => {
          let siteUrl = it.key;
          if (kind === "team") {
            try {
              siteUrl = await invoke<string>("browse_graph_team_site_url", { auth, teamGroupId: it.key });
            } catch (e) {
              alert("サイト解決に失敗: " + String(e));
              return;
            }
          }
          await openSpResourceStep(auth, siteUrl, it.label);
        };
        listEl.appendChild(row);
      }
    };
    filterEl.oninput = draw;
    filterEl.value = "";
    draw();
  }

  async function openSpResourceStep(
    auth: { tenant_id: string; client_id: string; client_secret: string },
    siteUrl: string,
    siteLabel: string,
  ) {
    $<HTMLDivElement>("sp-step-source").hidden = true;
    const stepRes = $<HTMLDivElement>("sp-step-resources");
    stepRes.hidden = false;
    $<HTMLDivElement>("sp-picker").querySelector(".sp-selected-site")!.textContent = `サイト: ${siteLabel} (${siteUrl})`;
    const resList = $<HTMLDivElement>("sp-resource-list");
    resList.innerHTML = "<div class='picker-empty'>読み込み中…</div>";
    const addBtn = $<HTMLButtonElement>("sp-picker-add");
    addBtn.hidden = false;

    const wantKind = $<HTMLSelectElement>("f-type").value === "graph_sharepoint_library" ? "library" : "list";
    try {
      const all = await invoke<{ display_name: string; kind: string }[]>("browse_graph_site_resources", { auth, siteUrl });
      const filtered = all.filter((r) => r.kind === wantKind);
      resList.innerHTML = "";
      if (filtered.length === 0) {
        resList.innerHTML = `<div class='picker-empty'>${wantKind === "library" ? "ライブラリ" : "リスト"}が見つかりませんでした</div>`;
      }
      for (const r of filtered) {
        const item = document.createElement("div");
        item.className = "picker-item";
        const cb = document.createElement("input");
        cb.type = "checkbox";
        cb.value = r.display_name;
        const span = document.createElement("span");
        span.textContent = r.display_name;
        item.append(cb, span);
        resList.appendChild(item);
      }
      addBtn.onclick = async () => {
        const checked = Array.from(resList.querySelectorAll("input:checked")) as HTMLInputElement[];
        if (checked.length === 0) { $<HTMLDialogElement>("sp-picker").close(); return; }
        const type_ = $<HTMLSelectElement>("f-type").value;
        for (const cb of checked) {
          const displayName = cb.value;
          const config: Record<string, unknown> =
            type_ === "graph_sharepoint_library"
              ? { ...auth, site_url: siteUrl, library_name: displayName }
              : { ...auth, site_url: siteUrl, list_name: displayName };
          try {
            await invoke("add_source", { type: type_, name: displayName, configJson: JSON.stringify(config) });
          } catch (e) {
            console.error(`追加失敗 (${displayName}):`, e);
          }
        }
        $<HTMLDialogElement>("sp-picker").close();
        $<HTMLDialogElement>("add-dialog").close();
        await loadSources();
      };
    } catch (e) {
      setPickerMessage(resList, `取得失敗: ${String(e)}`);
    }
  }

  async function openSharePointPicker() {
    const auth = await graphAuthFromDialog();
    if (!auth) { alert("先にトークンを選択してください（テナントID/クライアントID/クライアントシークレット）"); return; }
    spMode = "sites";
    $<HTMLDivElement>("sp-step-source").hidden = false;
    $<HTMLDivElement>("sp-step-resources").hidden = true;
    $<HTMLButtonElement>("sp-picker-add").hidden = true;
    $<HTMLDialogElement>("sp-picker").showModal();
    await loadSpSourceList(auth);
  }

  // ---- dialog close button handlers ----
  $<HTMLButtonElement>("notion-picker-close").onclick = () => $<HTMLDialogElement>("notion-picker").close();
  $<HTMLButtonElement>("browser-profile-picker-close").onclick = () => $<HTMLDialogElement>("browser-profile-picker").close();
  $<HTMLButtonElement>("sheet-picker-close").onclick = () => $<HTMLDialogElement>("sheet-picker").close();
  $<HTMLButtonElement>("airtable-picker-close").onclick = () => $<HTMLDialogElement>("airtable-picker").close();
  $<HTMLButtonElement>("cell-picker-close").onclick = () => $<HTMLDialogElement>("cell-picker").close();
  $<HTMLButtonElement>("table-picker-cancel").onclick = () => tablePickerDlg.close();
  $<HTMLButtonElement>("table-picker-all").onclick = () => {
    tablePickerList.querySelectorAll("input").forEach((cb) => ((cb as HTMLInputElement).checked = true));
    updateTablePickerCount();
  };
  $<HTMLButtonElement>("table-picker-none").onclick = () => {
    tablePickerList.querySelectorAll("input").forEach((cb) => ((cb as HTMLInputElement).checked = false));
    updateTablePickerCount();
  };
  $<HTMLButtonElement>("sp-tab-sites").onclick = async () => {
    spMode = "sites";
    $<HTMLButtonElement>("sp-tab-sites").classList.add("active");
    $<HTMLButtonElement>("sp-tab-teams").classList.remove("active");
    const auth = await graphAuthFromDialog();
    if (auth) await loadSpSourceList(auth);
  };
  $<HTMLButtonElement>("sp-tab-teams").onclick = async () => {
    spMode = "teams";
    $<HTMLButtonElement>("sp-tab-teams").classList.add("active");
    $<HTMLButtonElement>("sp-tab-sites").classList.remove("active");
    const auth = await graphAuthFromDialog();
    if (auth) await loadSpSourceList(auth);
  };
  $<HTMLButtonElement>("sp-picker-cancel").onclick = () => $<HTMLDialogElement>("sp-picker").close();

  return {
    openBrowserProfilePicker,
    openSheetPicker,
    openExcelTablePicker,
    openCellPicker,
    openAirtableBasePicker,
    openAirtableTablePicker,
    openAirtableViewPicker,
    openNotionPicker,
    openTablePicker,
    openSharePointPicker,
  };
}

import { $, setPickerMessage } from "../../app/dom";
import { invoke } from "../../app/ipc";
import { appState } from "../../app/state";
import { colLetterOf, formatSelectedColumns, parseColumnSelection } from "../../shared/excel-columns";
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

  /** テーブル設定ダイアログ（fd-）か接続先ダイアログ（pf-）のうち、描画されている方の設定欄。 */
  function dialogField(key: string): HTMLInputElement | null {
    return (document.getElementById(`fd-${key}`) ?? document.getElementById(`pf-${key}`)) as HTMLInputElement | null;
  }

  /** 選択値を入れた後、表示名の自動入力と列一覧の再取得をテーブル設定ダイアログへ依頼する。 */
  function refreshSourceEditor(): void {
    const sourceEditor = getSourceEditor();
    sourceEditor.autoFillDefaultName();
    sourceEditor.maybeFetchAndRenderFields();
  }

  interface ListPickerOptions<T> {
    dialogId: string;
    listId: string;
    /** 複数のピッカーで共有するダイアログの見出し。 */
    title?: { id: string; text: string };
    load: () => Promise<T[]>;
    emptyMessage: string;
    label: (item: T) => string;
    tooltip?: (item: T) => string;
    onPick: (item: T) => void;
  }

  /** 一覧を取得して 1 件選ばせるピッカー。選択するとダイアログを閉じる。 */
  async function openListPicker<T>(options: ListPickerOptions<T>): Promise<void> {
    if (options.title) {
      const titleEl = $<HTMLHeadingElement>(options.title.id);
      if (titleEl) titleEl.textContent = options.title.text;
    }
    const listEl = $<HTMLDivElement>(options.listId);
    listEl.innerHTML = "<div class='picker-empty'>読み込み中…</div>";
    const dlg = $<HTMLDialogElement>(options.dialogId);
    dlg.showModal();
    try {
      const items = await options.load();
      listEl.innerHTML = "";
      if (items.length === 0) setPickerMessage(listEl, options.emptyMessage);
      for (const entry of items) {
        const item = document.createElement("div");
        item.className = "picker-item";
        item.textContent = options.label(entry);
        if (options.tooltip) item.title = options.tooltip(entry);
        item.onclick = () => {
          options.onPick(entry);
          dlg.close();
        };
        listEl.appendChild(item);
      }
    } catch (e) {
      setPickerMessage(listEl, `取得失敗: ${String(e)}`);
    }
  }

  async function openNotionPicker() {
    const tokenEl = document.getElementById("fd-token") as HTMLInputElement | null;
    const credentialId = Number(tokenEl?.dataset.credRef);
    if (!Number.isSafeInteger(credentialId) || credentialId <= 0) {
      alert("先に保存済みトークンを選択してください");
      return;
    }
    await openListPicker({
      dialogId: "notion-picker",
      listId: "notion-picker-list",
      load: () => invoke("browse_notion_databases", { credentialId }),
      emptyMessage: "アクセス可能なデータベースがありません（インテグレーションに共有してください）",
      label: (db) => db.title || "(無題)",
      onPick: (db) => getSourceEditor().applyNotionSelection(db.id, db.title || "(無題)"),
    });
  }

  async function openBrowserProfilePicker(target: "history" | "bookmarks", pathInput: HTMLInputElement) {
    const browserSel = document.getElementById("fd-browser") as HTMLSelectElement | null;
    const browser = browserSel?.value ?? "chrome";
    await openListPicker({
      dialogId: "browser-profile-picker",
      listId: "browser-profile-picker-list",
      load: () => invoke("browse_browser_profiles", { browser, target }),
      emptyMessage: "候補が見つかりませんでした",
      label: (entry) => entry.label,
      tooltip: (entry) => entry.path,
      onPick: (entry) => {
        pathInput.value = entry.path;
        const sourceEditor = getSourceEditor();
        sourceEditor.maybeAutoFillName(entry.path);
        sourceEditor.maybeFetchAndRenderFields();
      },
    });
  }

  /** Excel ファイル内のシート名・テーブル(ListObject)名を共通のダイアログで選ばせる。 */
  async function openExcelNamePicker(
    input: HTMLInputElement,
    command: "browse_excel_sheets" | "browse_excel_tables",
    title: string,
    emptyMessage: string,
  ) {
    const path = getTargetExcelPath();
    if (!path) {
      alert("先にファイルパスを入力・選択してください");
      return;
    }
    await openListPicker({
      dialogId: "sheet-picker",
      listId: "sheet-picker-list",
      title: { id: "sheet-picker-title", text: title },
      load: () => invoke(command, { path }),
      emptyMessage,
      label: (name) => name,
      onPick: (name) => {
        input.value = name;
        refreshSourceEditor();
      },
    });
  }

  function openSheetPicker(sheetInput: HTMLInputElement) {
    return openExcelNamePicker(sheetInput, "browse_excel_sheets", "シートを選択", "シートが見つかりませんでした");
  }

  function openExcelTablePicker(tableInput: HTMLInputElement) {
    return openExcelNamePicker(tableInput, "browse_excel_tables", "テーブル(ListObject)を選択", "テーブルが見つかりませんでした");
  }

  /** Airtable の一覧取得に使う接続情報。保存済みトークンを優先し、無ければ編集中または同じ接続先のソース設定を使う。 */
  function airtableBrowseTarget(): { credentialId: number } | { id: number; overridesJson: string } {
    const tokenEl = dialogField("token");
    if (tokenEl?.dataset.credRef) return { credentialId: Number(tokenEl.dataset.credRef) };
    const sourceEditor = getSourceEditor();
    const parentSettings = getParentSettings();
    const sid = sourceEditor.getEditingId() ?? parentSettings.getSiblingSourceId();
    if (sid) {
      const overridesJson = sourceEditor.getEditingId() !== null
        ? JSON.stringify(sourceEditor.getConfigFromFields().config)
        : JSON.stringify(parentSettings.getParentSharedValues());
      return { id: sid, overridesJson };
    }
    alert("先にトークンを選択してください");
    throw new Error("トークン未選択");
  }

  async function fetchAirtableBasesForPicker(): Promise<AirtableBaseItem[]> {
    const target = airtableBrowseTarget();
    return "credentialId" in target
      ? await invoke("browse_airtable_bases", target)
      : await invoke("browse_airtable_bases_for_source", target);
  }

  async function fetchAirtableTablesForPicker(): Promise<AirtableTableItem[]> {
    const baseId = dialogField("base_id")?.value.trim() ?? "";
    if (!baseId) {
      alert("先に Base ID を入力・選択してください");
      throw new Error("Base ID 必須");
    }
    const target = airtableBrowseTarget();
    return "credentialId" in target
      ? await invoke("browse_airtable_tables", { ...target, baseId })
      : await invoke("browse_airtable_tables_for_source", { ...target, baseId });
  }

  /** Airtable の Base / テーブル / ビューを選ばせ、ID と確認用の名前を設定欄へ入れる。 */
  function openAirtablePicker<T extends { id: string; name: string }>(
    input: HTMLInputElement,
    options: { title: string; nameKey: string; load: () => Promise<T[]>; emptyMessage: string; label: (item: T) => string },
  ) {
    return openListPicker({
      dialogId: "airtable-picker",
      listId: "airtable-picker-list",
      title: { id: "airtable-picker-title", text: options.title },
      load: options.load,
      emptyMessage: options.emptyMessage,
      label: (item) => item.name ? options.label(item) : item.id,
      onPick: (item) => {
        input.value = item.id;
        const nameEl = dialogField(options.nameKey);
        if (nameEl) nameEl.value = item.name;
        refreshSourceEditor();
      },
    });
  }

  async function openAirtableBasePicker(baseInput: HTMLInputElement) {
    await openAirtablePicker(baseInput, {
      title: "Airtable Base を選択",
      nameKey: "base_name",
      load: fetchAirtableBasesForPicker,
      emptyMessage: "利用可能な Base が見つかりませんでした（権限や schema.bases:read スコープを確認してください）",
      label: (b) => `${b.name} [ID: ${b.id}]`,
    });
  }

  async function openAirtableTablePicker(tableInput: HTMLInputElement) {
    await openAirtablePicker(tableInput, {
      title: "Airtable テーブルを選択",
      nameKey: "table_name",
      load: fetchAirtableTablesForPicker,
      emptyMessage: "テーブルが見つかりませんでした",
      label: (t) => `${t.name} [ID: ${t.id}]`,
    });
  }

  async function openAirtableViewPicker(viewInput: HTMLInputElement) {
    const tableEl = document.getElementById("fd-table") as HTMLInputElement | null;
    const tableVal = tableEl?.value.trim() ?? "";
    if (!tableVal) {
      alert("先にテーブルを指定・選択してください");
      return;
    }
    const tableKey = tableVal.toLowerCase();
    await openAirtablePicker(viewInput, {
      title: "Airtable ビューを選択",
      nameKey: "view_name",
      load: async () => {
        const tables = await fetchAirtableTablesForPicker();
        const table = tables.find((t) => t.id === tableVal || t.name === tableVal || t.id.toLowerCase() === tableKey || t.name.toLowerCase() === tableKey);
        return table?.views ?? [];
      },
      emptyMessage: "このテーブルに選択可能なビューが見つかりませんでした",
      label: (v) => `${v.name} [ID: ${v.id} / 種別: ${v.type_}]`,
    });
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
      const res = await invoke("browse_excel_grid", { path, sheet, rows: 10, cols: 10, mode: typeof gridMode === "string" ? gridMode : "top" });
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
      setPickerMessage(wrapEl, `取得失敗: ${String(e)}`);
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
      names = await invoke(cmd, { path });
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
      const type_ = browse;
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


  interface SpPickerRow {
    id: string;
    display_name: string;
    description: string;
    created_date_time: string;
    key: string;
  }

  type SpSortKey = "display_name" | "id" | "description" | "created_date_time";
  let spSortKey: SpSortKey = "display_name";
  let spSortAsc = true;
  let spPickerRows: SpPickerRow[] = [];
  let spPickerCredentialId: number | null = null;
  let spPickerKind: "site" | "team" = "site";

  function graphCredentialIdFromDialog(): number | null {
    const secretEl = document.getElementById("fd-client_secret") as HTMLInputElement | null;
    const credentialId = Number(secretEl?.dataset.credRef);
    return Number.isSafeInteger(credentialId) && credentialId > 0 ? credentialId : null;
  }

  function formatSpCreatedAt(iso: string): string {
    if (!iso) return "";
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) return iso;
    return d.toLocaleDateString("ja-JP");
  }

  function compareSpRows(a: SpPickerRow, b: SpPickerRow): number {
    if (spSortKey === "created_date_time") {
      const ad = Date.parse(a.created_date_time) || 0;
      const bd = Date.parse(b.created_date_time) || 0;
      return spSortAsc ? ad - bd : bd - ad;
    }
    const av = (a[spSortKey] || "").toLowerCase();
    const bv = (b[spSortKey] || "").toLowerCase();
    const cmp = av.localeCompare(bv, "ja");
    return spSortAsc ? cmp : -cmp;
  }

  function renderSpSourceTable() {
    const listEl = $<HTMLDivElement>("sp-source-list");
    const filterEl = $<HTMLInputElement>("sp-filter");
    const q = filterEl.value.trim().toLowerCase();
    const filtered = spPickerRows.filter((row) => {
      if (!q) return true;
      const hay = [row.display_name, row.id, row.description, row.key].join(" ").toLowerCase();
      return hay.includes(q);
    });
    filtered.sort(compareSpRows);

    listEl.innerHTML = "";
    const table = document.createElement("table");
    table.className = "picker-table";
    const thead = document.createElement("thead");
    const headerRow = document.createElement("tr");
    const columns: { key: SpSortKey; label: string }[] = [
      { key: "display_name", label: "表示名" },
      { key: "id", label: "ID" },
      { key: "description", label: "概要" },
      { key: "created_date_time", label: "作成日" },
    ];
    for (const col of columns) {
      const th = document.createElement("th");
      th.className = "sortable" + (spSortKey === col.key ? (spSortAsc ? " sort-asc" : " sort-desc") : "");
      th.textContent = col.label;
      th.onclick = () => {
        if (spSortKey === col.key) spSortAsc = !spSortAsc;
        else {
          spSortKey = col.key;
          spSortAsc = true;
        }
        renderSpSourceTable();
      };
      headerRow.appendChild(th);
    }
    thead.appendChild(headerRow);
    table.appendChild(thead);

    const tbody = document.createElement("tbody");
    if (filtered.length === 0) {
      const tr = document.createElement("tr");
      const td = document.createElement("td");
      td.colSpan = 4;
      td.className = "picker-empty";
      td.textContent = "見つかりませんでした";
      tr.appendChild(td);
      tbody.appendChild(tr);
    } else {
      for (const row of filtered) {
        const tr = document.createElement("tr");
        tr.title = row.key;
        const nameTd = document.createElement("td");
        nameTd.textContent = row.display_name || row.key;
        const idTd = document.createElement("td");
        idTd.className = "col-id";
        idTd.textContent = row.id;
        idTd.title = row.id;
        const descTd = document.createElement("td");
        descTd.className = "col-desc";
        descTd.textContent = row.description;
        descTd.title = row.description;
        const dateTd = document.createElement("td");
        dateTd.textContent = formatSpCreatedAt(row.created_date_time);
        tr.append(nameTd, idTd, descTd, dateTd);
        tr.onclick = () => void onSpSourceRowClick(row);
        tbody.appendChild(tr);
      }
    }
    table.appendChild(tbody);
    listEl.appendChild(table);
  }

  async function onSpSourceRowClick(row: SpPickerRow) {
    if (spPickerCredentialId === null) return;
    let siteUrl = row.key;
    const displayName = row.display_name || siteUrl;
    if (spPickerKind === "team") {
      try {
        siteUrl = await invoke("browse_graph_team_site_url", { credentialId: spPickerCredentialId, teamGroupId: row.id });
      } catch (e) {
        alert("サイト解決に失敗: " + String(e));
        return;
      }
    }
    await getSourceEditor().applySharePointSiteSelection(siteUrl, displayName);
    $<HTMLDialogElement>("sp-picker").close();
  }

  async function loadSpSourceList(credentialId: number) {
    const listEl = $<HTMLDivElement>("sp-source-list");
    listEl.innerHTML = "<div class='picker-empty'>読み込み中…</div>";
    spPickerCredentialId = credentialId;
    spSortKey = "display_name";
    spSortAsc = true;
    try {
      if (spMode === "sites") {
        spPickerKind = "site";
        const sites = await invoke("browse_graph_sites", { credentialId });
        spPickerRows = sites.map((s) => ({
          id: s.id,
          display_name: s.display_name || s.web_url,
          description: s.description,
          created_date_time: s.created_date_time,
          key: s.web_url,
        }));
      } else {
        spPickerKind = "team";
        const teams = await invoke("browse_graph_teams", { credentialId });
        spPickerRows = teams.map((t) => ({
          id: t.id,
          display_name: t.display_name,
          description: t.description,
          created_date_time: t.created_date_time,
          key: t.id,
        }));
      }
      const filterEl = $<HTMLInputElement>("sp-filter");
      filterEl.oninput = () => renderSpSourceTable();
      filterEl.value = "";
      renderSpSourceTable();
    } catch (e) {
      setPickerMessage(listEl, `取得失敗: ${String(e)}`);
    }
  }

  async function openSharePointPicker() {
    const credentialId = graphCredentialIdFromDialog();
    if (credentialId === null) { alert("先に保存済みトークンを選択してください"); return; }
    spMode = "sites";
    $<HTMLDivElement>("sp-step-source").hidden = false;
    $<HTMLDivElement>("sp-step-resources").hidden = true;
    $<HTMLButtonElement>("sp-picker-add").hidden = true;
    $<HTMLDialogElement>("sp-picker").showModal();
    await loadSpSourceList(credentialId);
  }

  // ---- dialog close button handlers ----
  for (const dialogId of ["notion-picker", "browser-profile-picker", "sheet-picker", "airtable-picker", "cell-picker"]) {
    $<HTMLButtonElement>(`${dialogId}-close`).onclick = () => $<HTMLDialogElement>(dialogId).close();
  }
  $<HTMLButtonElement>("table-picker-cancel").onclick = () => tablePickerDlg.close();
  const setAllTablePickerChecks = (checked: boolean) => {
    tablePickerList.querySelectorAll("input").forEach((cb) => ((cb as HTMLInputElement).checked = checked));
    updateTablePickerCount();
  };
  $<HTMLButtonElement>("table-picker-all").onclick = () => setAllTablePickerChecks(true);
  $<HTMLButtonElement>("table-picker-none").onclick = () => setAllTablePickerChecks(false);
  const switchSpMode = async (mode: "sites" | "teams") => {
    spMode = mode;
    $<HTMLButtonElement>("sp-tab-sites").classList.toggle("active", mode === "sites");
    $<HTMLButtonElement>("sp-tab-teams").classList.toggle("active", mode === "teams");
    const credentialId = graphCredentialIdFromDialog();
    if (credentialId !== null) await loadSpSourceList(credentialId);
  };
  $<HTMLButtonElement>("sp-tab-sites").onclick = () => switchSpMode("sites");
  $<HTMLButtonElement>("sp-tab-teams").onclick = () => switchSpMode("teams");
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

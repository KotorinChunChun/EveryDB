import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
// ==== 状態 ====
let sources = [];
let selectedSourceIds = new Set();
let rows = [];
let columns = []; // 表示する列（Source + フィールド名の和集合）
let searchSeq = 0; // 送出したリクエストの通し番号
let lastAppliedSeq = -1; // 表示に反映済みの最大 seq（競合対策）
let selectedCell = null;
let mpStatus = { is_set: false, unlocked: false };
const ROW_H = 26; // px 固定行高（仮想スクロール用）
const COL_W = 200; // px 既定列幅
const SECRET_PLACEHOLDER = "••••••••";
/** ファイルパス両端のダブルクォート・空白を除去する（エクスプローラの「パスのコピー」対策）。 */
function normalizePath(v) {
    let s = (v ?? "").trim();
    if (s.length >= 2 && s.startsWith('"') && s.endsWith('"'))
        s = s.slice(1, -1);
    return s.trim();
}
// ==== DOM ヘルパー ====
const $ = (id) => document.getElementById(id);
/**
 * ピッカー等の一行メッセージを差し込む。
 * メッセージには外部 API のエラー本文が混ざりうるため、innerHTML ではなく
 * textContent で描画して HTML として解釈されないようにする。
 */
function setPickerMessage(el, message) {
    el.innerHTML = "";
    const d = document.createElement("div");
    d.className = "picker-empty";
    d.textContent = message;
    el.appendChild(d);
}
const searchInput = $("search");
const sourcesEl = $("sources");
const statusEl = $("status");
const sidebarEl = document.querySelector(".sidebar");
const sidebarResizerEl = $("sidebar-resizer");
const headEl = $("grid-head");
const viewportEl = $("grid-viewport");
const spacerEl = $("grid-spacer");
const rowsEl = $("grid-rows");
const resultInfo = $("result-info");
const openBtn = $("open");
// ============================================================
// マスターパスワード（要件 §9-4）
// ============================================================
async function refreshMpStatus() {
    mpStatus = await invoke("mp_status");
}
async function ensureMasterPasswordSetup() {
    await refreshMpStatus();
    if (mpStatus.is_set)
        return;
    const dlg = $("mp-setup-dialog");
    const err = $("mp-setup-error");
    const pw1 = $("mp-setup-pw1");
    const pw2 = $("mp-setup-pw2");
    dlg.showModal();
    await new Promise((resolve) => {
        $("mp-setup-form").onsubmit = async (e) => {
            e.preventDefault();
            err.textContent = "";
            if (pw1.value.length < 4) {
                err.textContent = "4 文字以上で入力してください";
                return;
            }
            if (pw1.value !== pw2.value) {
                err.textContent = "確認用パスワードが一致しません";
                return;
            }
            try {
                await invoke("mp_set", { password: pw1.value });
                await refreshMpStatus();
                dlg.close();
                resolve();
            }
            catch (e2) {
                err.textContent = String(e2);
            }
        };
    });
}
/** シークレットを表示/使用する操作の前に解錠を保証する。解錠済みなら即 true。 */
async function ensureUnlocked() {
    await refreshMpStatus();
    if (mpStatus.unlocked)
        return true;
    const dlg = $("mp-verify-dialog");
    const err = $("mp-verify-error");
    const pw = $("mp-verify-pw");
    pw.value = "";
    err.textContent = "";
    dlg.showModal();
    return new Promise((resolve) => {
        $("mp-verify-form").onsubmit = async (e) => {
            e.preventDefault();
            const submitter = e.submitter;
            if (submitter?.value === "cancel") {
                dlg.close();
                resolve(false);
                return;
            }
            try {
                const ok = await invoke("mp_verify", { password: pw.value });
                if (ok) {
                    await refreshMpStatus();
                    dlg.close();
                    resolve(true);
                }
                else {
                    err.textContent = "パスワードが違います";
                }
            }
            catch (e2) {
                err.textContent = String(e2);
            }
        };
        $("mp-verify-dialog").querySelector('[value="cancel"]').addEventListener("click", () => resolve(false), { once: true });
    });
}
// ============================================================
// データソース一覧（サイドバー）
// ============================================================
async function loadSources() {
    sources = await invoke("list_sources");
    if (selectedSourceIds.size === 0)
        sources.forEach((s) => selectedSourceIds.add(s.id));
    renderSources();
    runSearch();
}
async function refreshSidebar() {
    sources = await invoke("list_sources");
    renderSources();
}
let dragSrcId = null;
const collapsedParents = new Set();
let sourceFilter = "";
/** 接続先キーの表示ラベル（パスは basename、URL 等はそのまま）。 */
/** 接続先（ツリーの親）ノードのユーザー設定表示名（parentKey -> ラベル）。未設定なら自動導出する。 */
let parentLabelOverrides = new Map();
async function loadParentLabels() {
    try {
        const entries = await invoke("list_parent_labels");
        parentLabelOverrides = new Map(entries);
    }
    catch { /* noop */ }
}
function parentLabel(parent) {
    const override = parentLabelOverrides.get(parent);
    if (override)
        return override;
    if (/[\\/]/.test(parent)) {
        const parts = parent.split(/[\\/]/).filter(Boolean);
        return parts[parts.length - 1] || parent;
    }
    return parent;
}
/** 1 つのテーブル（または単独の接続先）の行を作る。 */
function makeSourceItem(s) {
    const item = document.createElement("div");
    item.className = "source-item";
    item.draggable = true;
    item.dataset.id = String(s.id);
    const dot = document.createElement("span");
    dot.className = "dot " + (s.status === "ok" ? "ok" : s.status === "error" ? "err" : "idle");
    dot.title = s.last_error ?? s.status ?? "未更新";
    const cb = document.createElement("input");
    cb.type = "checkbox";
    cb.checked = selectedSourceIds.has(s.id);
    cb.id = `src-${s.id}`;
    cb.onclick = (ev) => {
        if (ev.altKey) {
            // Alt+クリック: これだけ ON、他は全解除（ソロ選択）。
            selectedSourceIds = new Set([s.id]);
            renderSources();
            runSearch();
            return;
        }
        if (cb.checked)
            selectedSourceIds.add(s.id);
        else
            selectedSourceIds.delete(s.id);
        renderSources();
        runSearch();
    };
    const label = document.createElement("label");
    label.htmlFor = cb.id;
    label.textContent = s.name;
    label.title = s.name;
    const count = document.createElement("span");
    count.className = "count";
    count.textContent = String(s.record_count);
    const reloadBtn = document.createElement("button");
    reloadBtn.className = "mini-btn";
    reloadBtn.title = "このソースだけ再読込";
    reloadBtn.textContent = "⟳";
    reloadBtn.onclick = async (ev) => { ev.stopPropagation(); await reloadOne(s.id); };
    item.append(dot, cb, label, count, reloadBtn);
    item.addEventListener("contextmenu", (ev) => {
        ev.preventDefault();
        openContextMenu(ev, s.id);
    });
    // ---- ドラッグ並び替え ----
    item.addEventListener("dragstart", () => { dragSrcId = s.id; item.classList.add("dragging"); });
    item.addEventListener("dragend", () => item.classList.remove("dragging"));
    item.addEventListener("dragover", (ev) => { ev.preventDefault(); item.classList.add("drag-over"); });
    item.addEventListener("dragleave", () => item.classList.remove("drag-over"));
    item.addEventListener("drop", async (ev) => {
        ev.preventDefault();
        item.classList.remove("drag-over");
        if (dragSrcId === null || dragSrcId === s.id)
            return;
        const order = sources.map((x) => x.id);
        const from = order.indexOf(dragSrcId);
        const to = order.indexOf(s.id);
        order.splice(to, 0, order.splice(from, 1)[0]);
        await invoke("reorder_sources", { orderedIds: order });
        await refreshSidebar();
    });
    return item;
}
function renderSources() {
    sourcesEl.innerHTML = "";
    const q = sourceFilter.trim().toLowerCase();
    const match = (s) => !q || s.name.toLowerCase().includes(q) || parentLabel(s.parent).toLowerCase().includes(q);
    // parent キーごとにグルーピング（出現順を維持）。parent="" は単独ノード。
    const groups = new Map();
    const standalone = [];
    for (const s of sources) {
        if (!s.parent) {
            standalone.push(s);
            continue;
        }
        if (!groups.has(s.parent))
            groups.set(s.parent, []);
        groups.get(s.parent).push(s);
    }
    // 出現順に、単独ノードとグループを混在させて描画する。
    const emitted = new Set();
    for (const s of sources) {
        if (!s.parent) {
            if (match(s))
                sourcesEl.appendChild(makeSourceItem(s));
            continue;
        }
        if (emitted.has(s.parent))
            continue;
        emitted.add(s.parent);
        const children = groups.get(s.parent);
        const visibleChildren = children.filter(match);
        if (visibleChildren.length === 0)
            continue;
        // 接続先ノード（tri-state チェックボックス + 開閉）。
        const selCount = children.filter((c) => selectedSourceIds.has(c.id)).length;
        const collapsed = collapsedParents.has(s.parent);
        const parentRow = document.createElement("div");
        parentRow.className = "tree-parent";
        const twisty = document.createElement("span");
        twisty.className = "twisty";
        twisty.textContent = collapsed ? "▶" : "▼";
        const pcb = document.createElement("input");
        pcb.type = "checkbox";
        pcb.checked = selCount === children.length;
        pcb.indeterminate = selCount > 0 && selCount < children.length;
        pcb.onclick = (ev) => {
            ev.stopPropagation();
            const turnOn = !(selCount === children.length);
            for (const c of children) {
                if (turnOn)
                    selectedSourceIds.add(c.id);
                else
                    selectedSourceIds.delete(c.id);
            }
            renderSources();
            runSearch();
        };
        const plabel = document.createElement("span");
        plabel.className = "p-label";
        plabel.textContent = parentLabel(s.parent);
        plabel.title = s.parent;
        const pcount = document.createElement("span");
        pcount.className = "count";
        pcount.textContent = `${children.length}`;
        parentRow.append(twisty, pcb, plabel, pcount);
        parentRow.onclick = () => {
            if (collapsed)
                collapsedParents.delete(s.parent);
            else
                collapsedParents.add(s.parent);
            renderSources();
        };
        parentRow.addEventListener("contextmenu", (ev) => {
            ev.preventDefault();
            openParentContextMenu(ev, s.parent);
        });
        sourcesEl.appendChild(parentRow);
        const childWrap = document.createElement("div");
        childWrap.className = "tree-children";
        childWrap.hidden = collapsed && !q; // 絞り込み中は展開して見せる
        for (const c of visibleChildren)
            childWrap.appendChild(makeSourceItem(c));
        sourcesEl.appendChild(childWrap);
    }
    updateStatus();
}
async function reloadOne(id) {
    resultInfo.textContent = "リロード中…";
    try {
        await invoke("reload_sources", { sourceIds: [id] });
    }
    catch (e) {
        resultInfo.textContent = "リロード失敗: " + String(e);
    }
    await refreshSidebar();
    await runSearch();
}
function updateStatus() {
    const errs = sources.filter((s) => s.status === "error");
    if (errs.length) {
        statusEl.textContent = "更新エラー:\n" + errs.map((s) => `・${s.name}: ${s.last_error ?? ""}`).join("\n");
    }
    else {
        const last = sources.map((s) => s.last_success_at).filter(Boolean).sort().pop();
        statusEl.textContent = last ? `最終更新: ${last}` : "まだ更新されていません";
    }
}
$("source-filter").addEventListener("input", (e) => {
    sourceFilter = e.target.value;
    renderSources();
});
$("select-all").onclick = () => {
    selectedSourceIds = new Set(sources.map((s) => s.id));
    renderSources();
    runSearch();
};
$("select-none").onclick = () => {
    selectedSourceIds = new Set();
    renderSources();
    runSearch();
};
// ============================================================
// 右クリックコンテキストメニュー（要件 §9-1）
// ============================================================
const ctxMenu = $("ctx-menu");
const gridCtxMenu = $("grid-ctx-menu");
const parentCtxMenu = $("parent-ctx-menu");
let ctxTargetId = null;
let parentCtxTargetKey = null;
function openContextMenu(ev, id) {
    closeAllContextMenus();
    ctxTargetId = id;
    const src = sources.find((s) => s.id === id);
    const openTableBtn = ctxMenu.querySelector('[data-action="open-table"]');
    openTableBtn.hidden = !(src && TABLE_OPENABLE_TYPES.has(src.type_));
    ctxMenu.style.left = ev.clientX + "px";
    ctxMenu.style.top = ev.clientY + "px";
    ctxMenu.hidden = false;
}
function openGridContextMenu(ev) {
    closeAllContextMenus();
    gridCtxMenu.style.left = ev.clientX + "px";
    gridCtxMenu.style.top = ev.clientY + "px";
    gridCtxMenu.hidden = false;
}
function openParentContextMenu(ev, parentKey) {
    closeAllContextMenus();
    parentCtxTargetKey = parentKey;
    parentCtxMenu.style.left = ev.clientX + "px";
    parentCtxMenu.style.top = ev.clientY + "px";
    parentCtxMenu.hidden = false;
}
function closeContextMenu() {
    ctxMenu.hidden = true;
    ctxTargetId = null;
}
function closeAllContextMenus() {
    ctxMenu.hidden = true;
    ctxTargetId = null;
    gridCtxMenu.hidden = true;
    parentCtxMenu.hidden = true;
    parentCtxTargetKey = null;
}
// 欄外クリック・別の場所での右クリック・ESC・スクロールで閉じる。
document.addEventListener("click", () => closeAllContextMenus());
document.addEventListener("contextmenu", (ev) => {
    // メニュー対象（テーブル項目 / グリッドセル / 接続先ノード）以外での右クリックは閉じる。
    const t = ev.target;
    if (!t.closest(".source-item") && !t.closest(".cell") && !t.closest(".tree-parent"))
        closeAllContextMenus();
});
document.addEventListener("keydown", (ev) => { if (ev.key === "Escape")
    closeAllContextMenus(); });
window.addEventListener("blur", () => closeAllContextMenus());
gridCtxMenu.addEventListener("click", async (ev) => {
    const btn = ev.target.closest("button");
    if (!btn)
        return;
    closeAllContextMenus();
    switch (btn.dataset.copy) {
        case "value":
            await copySelected();
            break;
        case "tsv":
            await copyRowAsTsv();
            break;
        case "tsv-header":
            await copyRowAsTsvWithHeader();
            break;
        case "json":
            await copyRowAsJson();
            break;
    }
});
ctxMenu.addEventListener("click", async (ev) => {
    const btn = ev.target.closest("button");
    if (!btn || ctxTargetId === null)
        return;
    const id = ctxTargetId;
    const src = sources.find((s) => s.id === id);
    closeContextMenu(); // コマンド選択時は即座に閉じる
    switch (btn.dataset.action) {
        case "view":
            await openEditDialog(id);
            break;
        case "duplicate": {
            const name = prompt("複製後の表示名", `${src?.name ?? ""} のコピー`);
            if (!name)
                break;
            await invoke("duplicate_source", { id, newName: name });
            await loadSources();
            break;
        }
        case "reload":
            await reloadOne(id);
            break;
        case "open-table": {
            try {
                const detail = await invoke("get_source_detail", { id });
                const url = tableOpenUrl(detail.type_, detail.config);
                if (url) {
                    await invoke("open_ref", { target: url });
                }
                else {
                    resultInfo.textContent = "このテーブルは直接開けません（接続情報が不足しています）";
                }
            }
            catch (e) {
                resultInfo.textContent = "開けませんでした: " + String(e);
            }
            break;
        }
        case "delete":
            if (!confirm(`「${src?.name ?? ""}」を削除しますか？（キャッシュも消えます）`))
                break;
            await invoke("delete_source", { id });
            selectedSourceIds.delete(id);
            await loadSources();
            break;
    }
});
parentCtxMenu.addEventListener("click", async (ev) => {
    const btn = ev.target.closest("button");
    const parentKey = parentCtxTargetKey;
    if (!btn || parentKey === null)
        return;
    closeAllContextMenus();
    const children = sources.filter((s) => s.parent === parentKey);
    if (children.length === 0)
        return;
    switch (btn.dataset.action) {
        case "edit":
            await openParentSettingsDialog(parentKey);
            break;
        case "reload": {
            resultInfo.textContent = "リロード中…";
            try {
                await invoke("reload_sources", { sourceIds: children.map((c) => c.id) });
            }
            catch (e) {
                resultInfo.textContent = "リロード失敗: " + String(e);
            }
            await loadSources();
            await runSearch();
            break;
        }
        case "open": {
            const kind = parentKindOf(children[0].type_);
            if (kind !== "path" && kind !== "sharepoint") {
                resultInfo.textContent = "この接続先は直接開けません";
                break;
            }
            try {
                await invoke("open_ref", { target: parentKey });
            }
            catch (e) {
                resultInfo.textContent = "開けませんでした: " + String(e);
            }
            break;
        }
        case "delete": {
            const names = children.map((c) => c.name).join(", ");
            if (!confirm(`「${parentLabel(parentKey)}」配下の ${children.length} 個のテーブルをすべて削除しますか？\n（${names}）\nキャッシュも消えます。`))
                break;
            for (const c of children) {
                await invoke("delete_source", { id: c.id });
                selectedSourceIds.delete(c.id);
            }
            await loadSources();
            break;
        }
    }
});
// ============================================================
// 検索（debounce + seq 競合対策 + 履歴）
// ============================================================
let debounceTimer;
function scheduleSearch() {
    clearTimeout(debounceTimer);
    debounceTimer = window.setTimeout(runSearch, 150);
}
function currentRowLimit() {
    const v = parseInt($("row-limit").value, 10);
    return Number.isFinite(v) && v > 0 ? v : 10000;
}
async function runSearch() {
    const seq = ++searchSeq;
    const query = searchInput.value;
    const ids = Array.from(selectedSourceIds);
    let resp;
    try {
        resp = await invoke("search", { query, sourceIds: ids, seq, limit: currentRowLimit() });
    }
    catch (e) {
        resultInfo.textContent = "検索エラー: " + String(e);
        return;
    }
    if (resp.seq <= lastAppliedSeq)
        return; // 古い結果は破棄
    lastAppliedSeq = resp.seq;
    rows = resp.rows;
    rebuildColumns();
    selectedCell = null;
    updateOpenButton();
    renderHead();
    renderRows();
    resultInfo.textContent = `${rows.length} 件${resp.truncated ? "（上限で打ち切り）" : ""}`;
    if (query.trim())
        void invoke("add_search_history_entry", { query }).catch(() => { });
}
/** 列見出しドラッグで並べ替えた順序（セッション内保持。「テーブル」列は対象外で常に先頭）。 */
let gridColumnOrder = [];
function rebuildColumns() {
    const seen = new Set();
    const cols = [];
    for (const r of rows) {
        for (const k of Object.keys(r.fields)) {
            if (!seen.has(k)) {
                seen.add(k);
                cols.push(k);
            }
        }
    }
    const known = gridColumnOrder.filter((c) => cols.includes(c));
    const extra = cols.filter((c) => !gridColumnOrder.includes(c));
    columns = ["テーブル", "見出し", ...known, ...extra];
}
// ---- 検索履歴パネル ----
const historyPanel = $("history-panel");
async function showHistory() {
    let items = [];
    try {
        items = await invoke("list_search_history");
    }
    catch { /* noop */ }
    historyPanel.innerHTML = "";
    if (items.length === 0) {
        const empty = document.createElement("div");
        empty.className = "history-empty";
        empty.textContent = "検索履歴はありません";
        historyPanel.appendChild(empty);
    }
    else {
        for (const q of items) {
            const it = document.createElement("div");
            it.className = "history-item";
            it.textContent = q;
            it.onmousedown = (ev) => {
                ev.preventDefault();
                searchInput.value = q;
                historyPanel.hidden = true;
                runSearch();
            };
            historyPanel.appendChild(it);
        }
    }
    historyPanel.hidden = false;
}
searchInput.addEventListener("focus", () => { if (!searchInput.value)
    void showHistory(); });
searchInput.addEventListener("input", () => { historyPanel.hidden = true; scheduleSearch(); });
searchInput.addEventListener("blur", () => { setTimeout(() => (historyPanel.hidden = true), 150); });
const presetPanel = $("preset-panel");
$("preset-save").onclick = async () => {
    const name = prompt("お気に入り名", searchInput.value || "無題の検索");
    if (!name)
        return;
    try {
        await invoke("save_search_preset", {
            name,
            query: searchInput.value,
            sourceIds: Array.from(selectedSourceIds),
        });
        resultInfo.textContent = `お気に入りに保存しました: ${name}`;
    }
    catch (e) {
        alert("保存に失敗: " + String(e));
    }
};
async function showPresetPanel() {
    let items = [];
    try {
        items = await invoke("list_search_presets");
    }
    catch { /* noop */ }
    presetPanel.innerHTML = "";
    if (items.length === 0) {
        const empty = document.createElement("div");
        empty.className = "history-empty";
        empty.textContent = "お気に入りはまだありません";
        presetPanel.appendChild(empty);
    }
    else {
        for (const p of items) {
            const row = document.createElement("div");
            row.className = "history-item preset-item";
            const label = document.createElement("span");
            label.className = "preset-label";
            label.textContent = p.query ? `${p.name}（${p.query}）` : p.name;
            label.onmousedown = (ev) => {
                ev.preventDefault();
                applyPreset(p);
                presetPanel.hidden = true;
            };
            const delBtn = document.createElement("button");
            delBtn.className = "preset-del";
            delBtn.textContent = "×";
            delBtn.title = "削除";
            delBtn.onmousedown = async (ev) => {
                ev.preventDefault();
                ev.stopPropagation();
                await invoke("delete_search_preset", { id: p.id });
                await showPresetPanel();
            };
            row.append(label, delBtn);
            presetPanel.appendChild(row);
        }
    }
    presetPanel.hidden = false;
}
function applyPreset(p) {
    searchInput.value = p.query;
    selectedSourceIds = new Set(p.source_ids.filter((id) => sources.some((s) => s.id === id)));
    renderSources();
    runSearch();
}
$("preset-list-btn").onclick = (ev) => {
    ev.stopPropagation();
    if (!presetPanel.hidden) {
        presetPanel.hidden = true;
        return;
    }
    const rect = ($("preset-list-btn")).getBoundingClientRect();
    presetPanel.style.left = rect.left + "px";
    presetPanel.style.top = rect.bottom + 4 + "px";
    void showPresetPanel();
};
document.addEventListener("click", (ev) => {
    if (!ev.target.closest("#preset-panel, #preset-list-btn"))
        presetPanel.hidden = true;
});
// ============================================================
// グリッド描画（仮想スクロール）
// ============================================================
// 列幅（列名 -> px）。未設定の列は DEFAULT_COL_W を使う。検索のたびに columns は再構築されるが
// 列名をキーにしているため、同名列の幅はセッション中は保持される。
const columnWidths = {};
const MIN_COL_W = 40;
function colWidth(name) {
    return columnWidths[name] ?? COL_W;
}
function totalGridWidth() {
    return columns.reduce((sum, c) => sum + colWidth(c), 0);
}
function startColumnResize(ev, colName, handle) {
    ev.preventDefault();
    ev.stopPropagation();
    const startX = ev.clientX;
    const startW = colWidth(colName);
    handle.classList.add("dragging");
    document.body.style.cursor = "col-resize";
    function onMove(e) {
        columnWidths[colName] = Math.max(MIN_COL_W, startW + (e.clientX - startX));
        renderHead();
        renderRows();
    }
    function onUp() {
        document.removeEventListener("mousemove", onMove);
        document.removeEventListener("mouseup", onUp);
        document.body.style.cursor = "";
    }
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
}
let gridDragSrcCol = null;
/** テーブルの元フィールドに対応しない疑似列（常に先頭に固定・並べ替え/列対応編集の対象外）。 */
const PINNED_COLUMNS = new Set(["テーブル", "見出し"]);
function renderHead() {
    headEl.innerHTML = "";
    const inner = document.createElement("div");
    inner.className = "grid-head-inner";
    columns.forEach((c) => {
        const cell = document.createElement("div");
        cell.className = "cell head";
        cell.style.width = colWidth(c) + "px";
        const label = document.createElement("span");
        label.className = "cell-head-label";
        label.textContent = c;
        label.title = c;
        cell.appendChild(label);
        if (!PINNED_COLUMNS.has(c)) {
            cell.addEventListener("contextmenu", (ev) => {
                ev.preventDefault();
                openColumnContextMenu(ev, c);
            });
            // 列見出しのドラッグ並べ替え。
            cell.classList.add("draggable-col");
            cell.draggable = true;
            cell.addEventListener("dragstart", (ev) => {
                gridDragSrcCol = c;
                cell.classList.add("dragging");
                ev.dataTransfer?.setData("text/plain", c);
            });
            cell.addEventListener("dragend", () => cell.classList.remove("dragging"));
            cell.addEventListener("dragover", (ev) => {
                ev.preventDefault();
                cell.classList.add("drag-over");
            });
            cell.addEventListener("dragleave", () => cell.classList.remove("drag-over"));
            cell.addEventListener("drop", (ev) => {
                ev.preventDefault();
                cell.classList.remove("drag-over");
                if (gridDragSrcCol === null || gridDragSrcCol === c)
                    return;
                const order = columns.filter((x) => !PINNED_COLUMNS.has(x));
                const from = order.indexOf(gridDragSrcCol);
                const to = order.indexOf(c);
                if (from === -1 || to === -1)
                    return;
                order.splice(to, 0, order.splice(from, 1)[0]);
                gridColumnOrder = order;
                renderHead();
                renderRows();
            });
        }
        const handle = document.createElement("div");
        handle.className = "col-resize-handle";
        handle.addEventListener("mousedown", (ev) => startColumnResize(ev, c, handle));
        // ハンドルは列の境界(=左隣の列のセルの右端)に重なる。ここでの右クリックが親要素(セル)の
        // contextmenu(=一つ左の列の設定)に伝播してしまうバグを防ぐため、ここで止める。
        handle.addEventListener("contextmenu", (ev) => {
            ev.preventDefault();
            ev.stopPropagation();
        });
        cell.appendChild(handle);
        inner.appendChild(cell);
    });
    inner.style.width = totalGridWidth() + "px";
    headEl.appendChild(inner);
}
function cellText(r, col) {
    if (col === 0)
        return r.source_name;
    if (col === 1)
        return r.label ?? "";
    return r.fields[columns[col]] ?? "";
}
function renderRows() {
    spacerEl.style.height = rows.length * ROW_H + "px";
    const scrollTop = viewportEl.scrollTop;
    const viewH = viewportEl.clientHeight;
    const start = Math.max(0, Math.floor(scrollTop / ROW_H) - 5);
    const end = Math.min(rows.length, Math.ceil((scrollTop + viewH) / ROW_H) + 5);
    rowsEl.style.transform = `translateY(${start * ROW_H}px)`;
    rowsEl.innerHTML = "";
    const totalW = totalGridWidth();
    for (let i = start; i < end; i++) {
        const r = rows[i];
        const rowEl = document.createElement("div");
        rowEl.className = "grid-row" + (i % 2 ? " alt" : "");
        rowEl.style.height = ROW_H + "px";
        rowEl.style.width = totalW + "px";
        for (let c = 0; c < columns.length; c++) {
            const cell = document.createElement("div");
            cell.className = "cell" + (selectedCell && selectedCell.row === i && selectedCell.col === c ? " sel" : "");
            cell.style.width = colWidth(columns[c]) + "px";
            cell.textContent = cellText(r, c);
            cell.onmousedown = () => selectCell(i, c);
            cell.addEventListener("contextmenu", (ev) => {
                ev.preventDefault();
                selectCell(i, c);
                openGridContextMenu(ev);
            });
            rowEl.appendChild(cell);
        }
        rowsEl.appendChild(rowEl);
    }
}
function selectCell(row, col) {
    selectedCell = { row, col };
    updateOpenButton();
    renderRows();
    $("grid").focus();
}
function updateOpenButton() {
    const r = selectedCell ? rows[selectedCell.row] : null;
    openBtn.disabled = !r || !r.open_ref;
}
viewportEl.addEventListener("scroll", () => {
    headEl.scrollLeft = viewportEl.scrollLeft; // 見出しを本体の横スクロールに追従させる
    renderRows();
});
window.addEventListener("resize", () => renderRows());
function ensureRowVisible(row) {
    const top = row * ROW_H;
    const bottom = top + ROW_H;
    if (top < viewportEl.scrollTop) {
        viewportEl.scrollTop = top;
    }
    else if (bottom > viewportEl.scrollTop + viewportEl.clientHeight) {
        viewportEl.scrollTop = bottom - viewportEl.clientHeight;
    }
}
let columnMapRows = [];
let columnMapColumnName = "";
async function openColumnContextMenu(_ev, columnName) {
    columnMapColumnName = columnName;
    const dlg = $("column-map-dialog");
    const hint = $("column-map-hint");
    $("column-map-title").textContent = `列「${columnName}」の設定`;
    $("cm-display-name").value = columnName;
    $("cm-visible").checked = true;
    $("column-map-rows").innerHTML = "";
    hint.textContent = "読み込み中…";
    dlg.showModal();
    const sourceIds = Array.from(new Set(rows.map((r) => r.source_id)));
    columnMapRows = [];
    for (const sid of sourceIds) {
        try {
            const detail = await invoke("get_source_detail", { id: sid });
            const fields = await invoke("preview_source_fields_by_id", { id: sid });
            const aliases = detail.config.column_aliases ?? {};
            let currentField = null;
            for (const f of fields) {
                if ((aliases[f] ?? f) === columnName) {
                    currentField = f;
                    break;
                }
            }
            const src = sources.find((s) => s.id === sid);
            columnMapRows.push({ sourceId: sid, sourceName: src?.name ?? `#${sid}`, detail, fields, currentField });
        }
        catch (e) {
            console.error(`列マッピング取得失敗 (source ${sid}):`, e);
        }
    }
    // 表示状態の初期値: 現在マッピングされているテーブルのいずれかで非表示なら未チェックにする。
    let anyMapped = false;
    let anyHidden = false;
    for (const r of columnMapRows) {
        if (!r.currentField)
            continue;
        anyMapped = true;
        const disp = r.detail.config.display_columns;
        if (disp && !disp.includes(r.currentField))
            anyHidden = true;
    }
    if (anyMapped)
        $("cm-visible").checked = !anyHidden;
    renderColumnMapRows();
    hint.textContent = columnMapRows.length ? "" : "対応するデータソースが見つかりませんでした";
}
function renderColumnMapRows() {
    const tbody = $("column-map-rows");
    tbody.innerHTML = "";
    for (const r of columnMapRows) {
        const tr = document.createElement("tr");
        const nameTd = document.createElement("td");
        nameTd.textContent = r.sourceName;
        const selTd = document.createElement("td");
        const sel = document.createElement("select");
        const noneOpt = document.createElement("option");
        noneOpt.value = "";
        noneOpt.textContent = "（この列に対応しない）";
        sel.appendChild(noneOpt);
        for (const f of r.fields) {
            const opt = document.createElement("option");
            opt.value = f;
            opt.textContent = f;
            sel.appendChild(opt);
        }
        sel.value = r.currentField ?? "";
        selTd.appendChild(sel);
        tr.append(nameTd, selTd);
        tbody.appendChild(tr);
    }
}
$("column-map-cancel").onclick = () => $("column-map-dialog").close();
$("column-map-close-x").onclick = () => $("column-map-dialog").close();
$("column-map-save").onclick = async () => {
    const newName = $("cm-display-name").value.trim() || columnMapColumnName;
    const visible = $("cm-visible").checked;
    const selects = Array.from($("column-map-rows").querySelectorAll("select"));
    try {
        for (let i = 0; i < columnMapRows.length; i++) {
            const r = columnMapRows[i];
            const newField = selects[i].value || null;
            if (newField === null && r.currentField === null)
                continue; // 対応なし→対応なしは変更不要
            const cfg = { ...r.detail.config };
            const aliases = { ...(cfg.column_aliases ?? {}) };
            if (r.currentField && aliases[r.currentField] === columnMapColumnName) {
                delete aliases[r.currentField];
            }
            if (newField) {
                if (newField !== newName)
                    aliases[newField] = newName;
                else
                    delete aliases[newField];
            }
            cfg.column_aliases = aliases;
            if (newField) {
                let disp = cfg.display_columns;
                if (disp === undefined) {
                    if (!visible)
                        disp = r.fields.filter((f) => f !== newField);
                }
                else {
                    disp = disp.filter((f) => f !== newField);
                    if (visible)
                        disp.push(newField);
                }
                if (disp !== undefined)
                    cfg.display_columns = disp;
            }
            await invoke("update_source", { id: r.sourceId, name: r.detail.name, configJson: JSON.stringify(cfg) });
        }
        $("column-map-dialog").close();
        await loadSources();
        await runSearch();
    }
    catch (e) {
        alert("保存に失敗: " + String(e));
    }
};
// ============================================================
// 接続先（ツリーの親）右クリック: 接続設定の一括編集
// ============================================================
/** テキスト/パスワード系フィールドを任意のコンテナへ描画する簡易レンダラー（接続先ダイアログ用）。
 * renderTypeFields と異なり、共有(接続)フィールドは常に text/password + 任意の参照ボタンのみのため専用に用意する。 */
function renderFieldsInto(container, idPrefix, fields, prefill) {
    container.innerHTML = "";
    for (const f of fields) {
        const label = document.createElement("label");
        label.textContent = f.label;
        const input = document.createElement("input");
        input.type = f.kind === "password" ? "password" : "text";
        input.autocomplete = f.kind === "password" ? "off" : "on";
        input.id = `${idPrefix}${f.key}`;
        if (f.placeholder)
            input.placeholder = f.placeholder;
        if (prefill) {
            const v = prefill[f.key];
            if (v !== undefined && v !== null)
                input.value = String(v);
        }
        if (f.browse) {
            const row = document.createElement("div");
            row.className = "path-row";
            const browseBtn = document.createElement("button");
            browseBtn.type = "button";
            browseBtn.className = "btn browse-btn";
            browseBtn.textContent = "参照…";
            browseBtn.onclick = async () => {
                try {
                    const picked = await openDialog({
                        directory: f.browse === "folder",
                        multiple: false,
                        filters: f.browse === "file" ? f.dialogFilters : undefined,
                    });
                    if (typeof picked === "string")
                        input.value = picked;
                }
                catch (e) {
                    console.error("ファイル選択に失敗:", e);
                }
            };
            row.append(input, browseBtn);
            label.appendChild(row);
        }
        else {
            label.appendChild(input);
        }
        container.appendChild(label);
    }
}
/** 接続先ダイアログから配下テーブルを一覧取得して追加/削除できる種別。それ以外は非対応（個別に追加/削除）。 */
function parentBrowseKind(repType) {
    if (repType === "sqlite")
        return "sqlite";
    if (repType === "accdb")
        return "accdb";
    if (repType === "excel_table")
        return "excel_table";
    if (repType.startsWith("graph_sharepoint"))
        return "sharepoint";
    if (repType === "notion")
        return "notion";
    return null;
}
let parentDialogState = null;
const parentDynamicEl = $("parent-dynamic");
async function openParentSettingsDialog(parentKey) {
    const children = sources.filter((s) => s.parent === parentKey);
    if (children.length === 0)
        return;
    // 同じ接続先配下に複数の種別が混在する場合（例: 同一ブック内の excel_table と excel_sheets）は、
    // テーブル一覧の追加/削除に対応する種別を優先的に代表として選ぶ（表示種別・ボタン表示の一貫性のため）。
    const repChild = children.find((c) => parentBrowseKind(c.type_) !== null) ?? children[0];
    const repType = repChild.type_;
    const keys = sharedKeysFor(parentKindOf(repType));
    const def = TYPE_DEFS.find((t) => t.value === repType);
    const fieldDefs = (def?.fields ?? []).filter((f) => keys.includes(f.key));
    const errEl = $("parent-error");
    errEl.textContent = "";
    $("parent-dialog-title").textContent = `接続設定: ${parentLabel(parentKey)}`;
    $("parent-type-label").textContent = `種別: ${parentKindLabel(parentKindOf(repType), repType)}`;
    $("parent-label").value = parentLabelOverrides.get(parentKey) ?? "";
    $("parent-scope-hint").textContent =
        `この接続情報は次の ${children.length} 個のテーブルに反映されます: ${children.map((c) => c.name).join(", ")}`;
    let prefill = {};
    if (fieldDefs.length > 0) {
        try {
            const detail = await invoke("get_source_detail", { id: children[0].id });
            prefill = detail.config;
        }
        catch { /* noop */ }
    }
    renderFieldsInto(parentDynamicEl, "pf-", fieldDefs, prefill);
    const manageBtn = $("parent-manage-tables");
    manageBtn.hidden = parentBrowseKind(repType) === null;
    parentDialogState = { parentKey, keys, fieldDefs, children, repType };
    $("parent-dialog").showModal();
}
// ロック中でプレースホルダーが入っているパスワード欄をクリックしたら解錠を促す（本体ダイアログと同じ挙動）。
parentDynamicEl.addEventListener("focusin", async (ev) => {
    const el = ev.target;
    if (el.type !== "password" || el.value !== SECRET_PLACEHOLDER || !parentDialogState)
        return;
    const ok = await ensureUnlocked();
    if (ok) {
        const detail = await invoke("get_source_detail", { id: parentDialogState.children[0].id });
        el.value = String(detail.config[el.id.replace("pf-", "")] ?? "");
    }
});
$("parent-cancel").onclick = () => $("parent-dialog").close();
$("parent-close-x").onclick = () => $("parent-dialog").close();
$("parent-save").onclick = async () => {
    if (!parentDialogState)
        return;
    const { parentKey, keys, fieldDefs, children } = parentDialogState;
    const errEl = $("parent-error");
    errEl.textContent = "";
    const newLabel = $("parent-label").value.trim();
    const sharedValues = {};
    for (const k of keys) {
        const el = document.getElementById(`pf-${k}`);
        if (!el)
            continue;
        const fdef = fieldDefs.find((f) => f.key === k);
        const raw = k === "path" ? normalizePath(el.value) : el.value.trim();
        if (fdef?.optional && raw === "")
            continue; // 空の任意項目は既存値を維持（変更しない）
        sharedValues[k] = raw;
    }
    const requiredEmpty = keys.some((k) => {
        const fdef = fieldDefs.find((f) => f.key === k);
        if (fdef?.optional)
            return false;
        return !String(sharedValues[k] ?? "").trim();
    });
    if (fieldDefs.length > 0 && requiredEmpty) {
        errEl.textContent = "接続情報に未入力の項目があります";
        return;
    }
    try {
        await invoke("set_parent_label", { parentKey, label: newLabel });
        if (newLabel)
            parentLabelOverrides.set(parentKey, newLabel);
        else
            parentLabelOverrides.delete(parentKey);
        for (const c of children) {
            const detail = await invoke("get_source_detail", { id: c.id });
            const cfg = { ...detail.config };
            for (const k of keys) {
                if (!(k in sharedValues))
                    continue;
                cfg[k] = sharedValues[k];
            }
            await invoke("update_source", { id: c.id, name: detail.name, configJson: JSON.stringify(cfg) });
        }
        $("parent-dialog").close();
        await loadSources();
        await runSearch();
    }
    catch (e) {
        errEl.textContent = "保存に失敗: " + String(e);
    }
};
let parentTablePickerMatched = new Map(); // item.key -> 既存の子ソースID
let parentTablePickerItems = [];
let parentTablePickerSharedNow = {};
$("parent-manage-tables").onclick = () => void openParentTablePicker();
async function openParentTablePicker() {
    if (!parentDialogState)
        return;
    const { keys, children, repType } = parentDialogState;
    const browseKind = parentBrowseKind(repType);
    if (!browseKind)
        return;
    const dlg = $("parent-table-picker");
    const listEl = $("parent-table-picker-list");
    const errEl = $("parent-table-picker-error");
    errEl.textContent = "";
    listEl.innerHTML = "<div class='picker-empty'>読み込み中…</div>";
    dlg.showModal();
    // このダイアログでまだ保存していない編集中の接続情報も含めて使う。
    const sharedNow = {};
    for (const k of keys) {
        const el = document.getElementById(`pf-${k}`);
        if (el)
            sharedNow[k] = el.value;
    }
    parentTablePickerItems = [];
    try {
        if (browseKind === "sqlite" || browseKind === "accdb" || browseKind === "excel_table") {
            const cmd = browseKind === "sqlite" ? "browse_sqlite_tables" : browseKind === "accdb" ? "browse_accdb_tables" : "browse_excel_tables";
            const names = await invoke(cmd, { path: sharedNow.path });
            parentTablePickerItems = names.map((n) => ({ key: n, label: n, type_: repType, extraConfig: { table: n } }));
        }
        else if (browseKind === "sharepoint") {
            const auth = { tenant_id: sharedNow.tenant_id, client_id: sharedNow.client_id, client_secret: sharedNow.client_secret };
            const resources = await invoke("browse_graph_site_resources", { auth, siteUrl: sharedNow.site_url });
            parentTablePickerItems = resources.map((r) => ({
                key: `${r.kind}:${r.display_name}`,
                label: r.kind === "library" ? `${r.display_name}（ライブラリ）` : r.display_name,
                type_: r.kind === "library" ? "graph_sharepoint_library" : "graph_sharepoint_list",
                extraConfig: r.kind === "library" ? { library_name: r.display_name } : { list_name: r.display_name },
            }));
        }
        else if (browseKind === "notion") {
            const dbs = await invoke("browse_notion_databases", { token: sharedNow.token });
            parentTablePickerItems = dbs.map((d) => ({ key: d.id, label: d.title || "(無題)", type_: "notion", extraConfig: { database_id: d.id } }));
        }
    }
    catch (e) {
        setPickerMessage(listEl, `取得失敗: ${String(e)}`);
        return;
    }
    // 既存の配下テーブルとの対応付け（元フィールドの値で照合）。
    parentTablePickerMatched = new Map();
    for (const c of children) {
        try {
            const detail = await invoke("get_source_detail", { id: c.id });
            const cfg = detail.config;
            if (browseKind === "sqlite" || browseKind === "accdb" || browseKind === "excel_table") {
                if (typeof cfg.table === "string")
                    parentTablePickerMatched.set(cfg.table, c.id);
            }
            else if (browseKind === "sharepoint") {
                if (typeof cfg.list_name === "string")
                    parentTablePickerMatched.set(`list:${cfg.list_name}`, c.id);
                if (typeof cfg.library_name === "string")
                    parentTablePickerMatched.set(`library:${cfg.library_name}`, c.id);
            }
            else if (browseKind === "notion") {
                if (typeof cfg.database_id === "string")
                    parentTablePickerMatched.set(cfg.database_id, c.id);
            }
        }
        catch { /* このソースは対応付けをスキップ */ }
    }
    listEl.innerHTML = "";
    if (parentTablePickerItems.length === 0) {
        listEl.innerHTML = "<div class='picker-empty'>見つかりませんでした</div>";
    }
    for (const item of parentTablePickerItems) {
        const row = document.createElement("label");
        row.className = "picker-item";
        const cb = document.createElement("input");
        cb.type = "checkbox";
        cb.dataset.key = item.key;
        cb.checked = parentTablePickerMatched.has(item.key);
        const span = document.createElement("span");
        span.textContent = item.label;
        row.append(cb, span);
        listEl.appendChild(row);
    }
    // 適用時に使う共有接続情報を保持しておく（このスコープ外の apply ハンドラから参照するため）。
    parentTablePickerSharedNow = sharedNow;
}
$("parent-table-picker-all").onclick = () => {
    $("parent-table-picker-list").querySelectorAll("input[type=checkbox]").forEach((cb) => { cb.checked = true; });
};
$("parent-table-picker-none").onclick = () => {
    $("parent-table-picker-list").querySelectorAll("input[type=checkbox]").forEach((cb) => { cb.checked = false; });
};
$("parent-table-picker-cancel").onclick = () => $("parent-table-picker").close();
$("parent-table-picker-close-x").onclick = () => $("parent-table-picker").close();
$("parent-table-picker-apply").onclick = async () => {
    if (!parentDialogState)
        return;
    const { keys } = parentDialogState;
    const errEl = $("parent-table-picker-error");
    errEl.textContent = "";
    const checkboxes = Array.from($("parent-table-picker-list").querySelectorAll("input[type=checkbox]"));
    const failures = [];
    try {
        for (const cb of checkboxes) {
            const key = cb.dataset.key;
            const item = parentTablePickerItems.find((i) => i.key === key);
            if (!item)
                continue;
            if (cb.checked && !parentTablePickerMatched.has(key)) {
                const config = { ...item.extraConfig };
                for (const k of keys) {
                    if (!parentTablePickerSharedNow[k])
                        continue;
                    config[k] = k === "path" ? normalizePath(parentTablePickerSharedNow[k]) : parentTablePickerSharedNow[k];
                }
                try {
                    await invoke("add_source", { type: item.type_, name: item.label, configJson: JSON.stringify(config) });
                }
                catch (e) {
                    failures.push(`${item.label}: ${String(e)}`);
                }
            }
            else if (!cb.checked && parentTablePickerMatched.has(key)) {
                const id = parentTablePickerMatched.get(key);
                await invoke("delete_source", { id });
                selectedSourceIds.delete(id);
            }
        }
        if (failures.length) {
            errEl.textContent = `一部失敗しました: ${failures.join(" / ")}`;
            return;
        }
        $("parent-table-picker").close();
        await loadSources();
        await runSearch();
        if (parentDialogState)
            await openParentSettingsDialog(parentDialogState.parentKey);
    }
    catch (e) {
        errEl.textContent = "適用に失敗: " + String(e);
    }
};
async function copyToClipboard(text, label) {
    try {
        await navigator.clipboard.writeText(text);
        resultInfo.textContent = `${label}: ${text.slice(0, 60)}`;
    }
    catch {
        resultInfo.textContent = "クリップボードへのコピーに失敗";
    }
}
/** 値コピー（要件 REQ-CP-02: Ctrl+C の既定動作として維持）。 */
async function copySelected() {
    if (!selectedCell)
        return;
    await copyToClipboard(cellText(rows[selectedCell.row], selectedCell.col), "コピー");
}
/** 行を TSV でコピー（全列、タブ区切り）。要件 REQ-CP-01. */
async function copyRowAsTsv() {
    if (!selectedCell)
        return;
    const r = rows[selectedCell.row];
    const line = columns.map((_, c) => cellText(r, c)).join("\t");
    await copyToClipboard(line, "TSVコピー");
}
/** 見出し付き TSV（値が空の列は除外）。要件 REQ-CP-01. */
async function copyRowAsTsvWithHeader() {
    if (!selectedCell)
        return;
    const r = rows[selectedCell.row];
    const pairs = columns.map((c, i) => [c, cellText(r, i)]).filter(([, v]) => v !== "");
    const header = pairs.map(([k]) => k).join("\t");
    const values = pairs.map(([, v]) => v).join("\t");
    await copyToClipboard(`${header}\n${values}`, "見出し付きTSVコピー");
}
/** レコードを JSON でコピー。要件 REQ-CP-01. */
async function copyRowAsJson() {
    if (!selectedCell)
        return;
    const r = rows[selectedCell.row];
    const obj = {};
    columns.forEach((c, i) => { obj[c] = cellText(r, i); });
    await copyToClipboard(JSON.stringify(obj, null, 2), "JSONコピー");
}
async function openSelected() {
    if (!selectedCell)
        return;
    const r = rows[selectedCell.row];
    if (!r.open_ref)
        return;
    try {
        await invoke("open_ref", { target: r.open_ref });
    }
    catch (e) {
        resultInfo.textContent = "開けませんでした: " + String(e);
    }
}
$("grid").addEventListener("keydown", (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "c") {
        e.preventDefault();
        void copySelected();
        return;
    }
    if (rows.length === 0)
        return;
    if (!selectedCell && ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(e.key)) {
        selectCell(0, 0);
        e.preventDefault();
        return;
    }
    if (!selectedCell)
        return;
    let { row, col } = selectedCell;
    switch (e.key) {
        case "ArrowDown":
            row = Math.min(rows.length - 1, row + 1);
            break;
        case "ArrowUp":
            row = Math.max(0, row - 1);
            break;
        case "ArrowRight":
            col = Math.min(columns.length - 1, col + 1);
            break;
        case "ArrowLeft":
            col = Math.max(0, col - 1);
            break;
        case "Enter":
            e.preventDefault();
            void openSelected();
            return;
        default: return;
    }
    e.preventDefault();
    selectedCell = { row, col };
    ensureRowVisible(row);
    updateOpenButton();
    renderRows();
});
openBtn.onclick = () => void openSelected();
// ---- 即時リロード（設定ダイアログ内・全データソース対象。要件: リロード中は可視化する）----
let reloading = false;
async function reloadAllSources() {
    if (reloading)
        return;
    reloading = true;
    const btn = $("s-reload");
    const status = $("reload-status");
    btn.disabled = true;
    status.hidden = false;
    resultInfo.textContent = "リロード中…（完了まで操作をお待ちください）";
    try {
        await invoke("reload_sources", { sourceIds: [] });
    }
    catch (e) {
        resultInfo.textContent = "リロード失敗: " + String(e);
    }
    await loadSources();
    btn.disabled = false;
    status.hidden = true;
    reloading = false;
}
$("s-reload").onclick = () => void reloadAllSources();
// ---- 行数上限の変更で再検索 ----
$("row-limit").addEventListener("change", () => runSearch());
// ---- TSV エクスポート（現在の検索結果を保存）----
function tsvEscape(v) {
    // タブ・改行を空白に置換して 1 セル 1 値を保つ。
    return v.replace(/[\t\r\n]+/g, " ");
}
$("export").onclick = async () => {
    if (rows.length === 0) {
        resultInfo.textContent = "エクスポートする結果がありません";
        return;
    }
    const header = columns.map(tsvEscape).join("\t");
    const lines = rows.map((r) => columns.map((_, c) => tsvEscape(cellText(r, c))).join("\t"));
    const tsv = [header, ...lines].join("\r\n");
    try {
        const path = await saveDialog({
            defaultPath: "everydb_export.tsv",
            filters: [{ name: "TSV", extensions: ["tsv", "txt"] }],
        });
        if (!path)
            return;
        await writeTextFile(path, tsv);
        resultInfo.textContent = `エクスポート完了: ${path}（${rows.length} 件）`;
    }
    catch (e) {
        resultInfo.textContent = "エクスポート失敗: " + String(e);
    }
};
/** ファイルパス入力欄。browse時、種別に応じた拡張子でファイル選択ダイアログを絞り込む。 */
function pathField(filters) {
    return { key: "path", label: "ファイルパス", kind: "text", placeholder: "C:\\path\\to\\file", browse: "file", dialogFilters: filters };
}
const EXCEL_FILTERS = [{ name: "Excel ブック", extensions: ["xlsx", "xlsm", "xls"] }];
const CSV_FILTERS = [{ name: "CSV ファイル", extensions: ["csv"] }];
const SQLITE_FILTERS = [{ name: "SQLite データベース", extensions: ["sqlite", "sqlite3", "db", "db3"] }];
const ACCDB_FILTERS = [{ name: "Access データベース", extensions: ["accdb", "mdb"] }];
const TEXT_FILTERS = [{ name: "テキストファイル", extensions: ["txt"] }];
const JSON_FILTERS = [{ name: "JSON ファイル", extensions: ["json"] }];
const GRAPH_AUTH_FIELDS = [
    { key: "tenant_id", label: "テナントID", kind: "text", placeholder: "ディレクトリ(テナント) ID" },
    { key: "client_id", label: "クライアントID", kind: "text", placeholder: "アプリケーション(クライアント) ID" },
    { key: "client_secret", label: "クライアントシークレット(値)", kind: "password", placeholder: "シークレットの値(ID ではない)" },
];
const TYPE_DEFS = [
    { value: "csv", label: "CSV ファイル", fields: [
            pathField(CSV_FILTERS),
            { key: "skip_rows_top", label: "先頭から読み飛ばす行数", kind: "number", placeholder: "0", min: 0, optional: true },
            { key: "has_header", label: "（読み飛ばした後の）先頭行はヘッダー", kind: "checkbox", default: true },
            { key: "skip_rows_bottom", label: "末尾から読み飛ばす行数", kind: "number", placeholder: "0", min: 0, optional: true },
        ] },
    { value: "sqlite", label: "SQLite データベース", fields: [pathField(SQLITE_FILTERS), { key: "table", label: "テーブル名", kind: "text", placeholder: "customers" }], browse: "sqlite" },
    { value: "accdb", label: "Access (accdb / mdb)", fields: [pathField(ACCDB_FILTERS), { key: "table", label: "テーブル名", kind: "text", placeholder: "取引先" }], browse: "accdb" },
    { value: "excel_table", label: "Excel: テーブル(ListObject)", fields: [pathField(EXCEL_FILTERS), { key: "table", label: "テーブル名(ListObject)", kind: "text", placeholder: "テーブル1" }], browse: "excel_table" },
    { value: "excel_range", label: "Excel: シート(セル範囲)", fields: [pathField(EXCEL_FILTERS), { key: "sheet", label: "シート名", kind: "text" }, { key: "range", label: "セル範囲 (A1形式)", kind: "text", placeholder: "A1:C10" }, { key: "has_header", label: "範囲の先頭行はヘッダー", kind: "checkbox", default: true }] },
    { value: "excel_sheets", label: "Excel: シート名一覧", fields: [pathField(EXCEL_FILTERS)] },
    { value: "folder", label: "フォルダのファイル一覧", fields: [{ key: "path", label: "フォルダパス", kind: "text", placeholder: "C:\\path\\to\\folder", browse: "folder" }, { key: "recursive", label: "サブフォルダも含める", kind: "checkbox", default: true }, { key: "extensions", label: "拡張子フィルタ (カンマ区切り・任意)", kind: "list", placeholder: "csv, xlsx" }] },
    { value: "ime_msime", label: "IME辞書: Microsoft IME", fields: [pathField(TEXT_FILTERS)] },
    { value: "ime_google", label: "IME辞書: Google 日本語入力", fields: [pathField(TEXT_FILTERS)] },
    { value: "notion", label: "Notion データベース", fields: [{ key: "token", label: "インテグレーショントークン", kind: "password", placeholder: "ntn_..." }, { key: "database_id", label: "データベースID", kind: "text", placeholder: "URL 内の 32 桁 ID" }], browse: "notion" },
    { value: "graph_entra_users", label: "EntraID ユーザー一覧", fields: [...GRAPH_AUTH_FIELDS] },
    { value: "graph_entra_groups", label: "EntraID グループ一覧", fields: [...GRAPH_AUTH_FIELDS] },
    { value: "graph_teams", label: "Teams チーム一覧", fields: [...GRAPH_AUTH_FIELDS] },
    { value: "graph_sharepoint_list", label: "SharePoint リスト", fields: [...GRAPH_AUTH_FIELDS, { key: "site_url", label: "サイトURL", kind: "text", placeholder: "https://～/sites/サイト名" }, { key: "list_name", label: "リスト名", kind: "text", placeholder: "SampleList" }], browse: "sharepoint" },
    { value: "graph_sharepoint_library", label: "SharePoint ドキュメントライブラリ", fields: [...GRAPH_AUTH_FIELDS, { key: "site_url", label: "サイトURL", kind: "text", placeholder: "https://～/sites/サイト名" }, { key: "library_name", label: "ライブラリ名(空欄=既定)", kind: "text", placeholder: "空欄で既定ライブラリ", optional: true }], browse: "sharepoint" },
    { value: "graph_entra_devices", label: "EntraID デバイス一覧", fields: [...GRAPH_AUTH_FIELDS] },
    { value: "graph_intune_devices", label: "Intune デバイス一覧", fields: [...GRAPH_AUTH_FIELDS] },
    { value: "graph_licenses", label: "Microsoft 365 ライセンス一覧", fields: [...GRAPH_AUTH_FIELDS] },
    { value: "graph_calendar", label: "Outlook 予定表", fields: [...GRAPH_AUTH_FIELDS, { key: "user", label: "対象ユーザー(UPN/メール)", kind: "text", placeholder: "user@example.com" }] },
    { value: "graph_onedrive", label: "OneDrive", fields: [...GRAPH_AUTH_FIELDS, { key: "user", label: "対象ユーザー(UPN/メール)", kind: "text", placeholder: "user@example.com" }] },
    { value: "graph_planner", label: "Teams タスク & Planner", fields: [...GRAPH_AUTH_FIELDS, { key: "group_id", label: "対象チーム(グループ)のID", kind: "text", placeholder: "Entra管理センター等で確認したグループID" }] },
    { value: "browser_history", label: "ブラウザ: 閲覧履歴", fields: [
            { key: "browser", label: "ブラウザ", kind: "select", options: [
                    { value: "chrome", label: "Google Chrome" }, { value: "edge", label: "Microsoft Edge" },
                    { value: "brave", label: "Brave" }, { value: "firefox", label: "Firefox" },
                ] },
            { key: "path", label: "History / places.sqlite のパス", kind: "text", placeholder: "%LOCALAPPDATA%\\Google\\Chrome\\User Data\\Default\\History", browse: "file" },
        ] },
    { value: "browser_bookmarks", label: "ブラウザ: ブックマーク", fields: [
            { key: "browser", label: "ブラウザ", kind: "select", options: [
                    { value: "chrome", label: "Google Chrome" }, { value: "edge", label: "Microsoft Edge" },
                    { value: "brave", label: "Brave" }, { value: "firefox", label: "Firefox" },
                ] },
            { key: "path", label: "Bookmarks / places.sqlite のパス", kind: "text", placeholder: "%LOCALAPPDATA%\\Google\\Chrome\\User Data\\Default\\Bookmarks", browse: "file" },
        ] },
    { value: "google_sheets", label: "Google スプレッドシート", fields: [
            { key: "key_path", label: "サービスアカウント鍵(JSON)のパス", kind: "text", placeholder: "C:\\path\\to\\service-account.json", browse: "file", dialogFilters: JSON_FILTERS },
            { key: "spreadsheet_id", label: "スプレッドシートID", kind: "text", placeholder: "URL の /d/ と /edit の間の文字列" },
            { key: "range", label: "範囲(任意)", kind: "text", placeholder: "Sheet1!A1:D50 / 空欄=先頭シート全域", optional: true },
            { key: "has_header", label: "先頭行はヘッダー", kind: "checkbox", default: true },
        ] },
    { value: "ical_calendar", label: "Google カレンダー / iCal (.ics)", fields: [
            { key: "url", label: "公開 iCal URL", kind: "text", placeholder: "https://calendar.google.com/calendar/ical/.../public/basic.ics" },
        ] },
    { value: "airtable", label: "Airtable", fields: [
            { key: "token", label: "Personal Access Token", kind: "password", placeholder: "pat..." },
            { key: "base_id", label: "Base ID", kind: "text", placeholder: "app..." },
            { key: "table", label: "テーブル ID または名前", kind: "text", placeholder: "tbl... または テーブル名" },
            { key: "view", label: "ビュー(任意)", kind: "text", placeholder: "viw... または ビュー名", optional: true },
        ] },
    { value: "postgres", label: "PostgreSQL", fields: [
            { key: "host", label: "ホスト", kind: "text", placeholder: "localhost / xxx.rds.amazonaws.com" },
            { key: "port", label: "ポート", kind: "text", placeholder: "5432", optional: true },
            { key: "database", label: "データベース名", kind: "text" },
            { key: "schema", label: "スキーマ(任意)", kind: "text", placeholder: "public", optional: true },
            { key: "table", label: "テーブル名", kind: "text" },
            { key: "user", label: "ユーザー", kind: "text" },
            { key: "password", label: "パスワード", kind: "password" },
            { key: "sslmode", label: "SSLモード(任意)", kind: "select", optional: true, options: [
                    { value: "prefer", label: "prefer (可能なら暗号化・既定)" },
                    { value: "require", label: "require (暗号化を必須)" },
                    { value: "verify-full", label: "verify-full (証明書とホスト名を検証)" },
                    { value: "disable", label: "disable (暗号化しない・非推奨)" },
                ] },
            { key: "ssl_accept_invalid_certs", label: "自己署名証明書を許可(社内サーバー向け)", kind: "checkbox", default: false },
        ] },
    { value: "mysql", label: "MySQL", fields: [
            { key: "host", label: "ホスト", kind: "text", placeholder: "localhost" },
            { key: "port", label: "ポート", kind: "text", placeholder: "3306", optional: true },
            { key: "database", label: "データベース名", kind: "text" },
            { key: "table", label: "テーブル名", kind: "text" },
            { key: "user", label: "ユーザー", kind: "text" },
            { key: "password", label: "パスワード", kind: "password" },
            { key: "use_tls", label: "TLS で接続する(リモート接続時は必ず有効化)", kind: "checkbox", default: false },
            { key: "ssl_accept_invalid_certs", label: "自己署名証明書を許可(社内サーバー向け)", kind: "checkbox", default: false },
        ] },
    { value: "box_files", label: "Box (ファイル一覧)", fields: [
            { key: "client_id", label: "クライアントID", kind: "text" },
            { key: "client_secret", label: "クライアントシークレット", kind: "password" },
            { key: "enterprise_id", label: "Enterprise ID", kind: "text" },
            { key: "folder_id", label: "開始フォルダID(任意)", kind: "text", placeholder: "0 (ルート)", optional: true },
        ] },
];
const PATH_GROUPED_TYPES = new Set([
    "csv", "folder", "ime_msime", "ime_google", "excel_table", "excel_range", "excel_sheets",
    "sqlite", "accdb", "browser_history", "browser_bookmarks",
]);
function parentKindOf(type_) {
    if (type_ === "notion")
        return "notion";
    if (type_.startsWith("graph_sharepoint"))
        return "sharepoint";
    if (type_.startsWith("graph"))
        return "graph365";
    if (type_ === "postgres")
        return "postgres";
    if (type_ === "mysql")
        return "mysql";
    if (type_ === "box_files")
        return "box";
    if (PATH_GROUPED_TYPES.has(type_))
        return "path";
    return "none";
}
/** 接続先(接続設定)ダイアログに表示する、接続種別の閲覧用ラベル。 */
function parentKindLabel(kind, repType) {
    switch (kind) {
        case "path": return `ファイル（${TYPE_DEFS.find((t) => t.value === repType)?.label ?? repType}）`;
        case "sharepoint": return "SharePoint サイト";
        case "notion": return "Notion";
        case "graph365": return "Microsoft 365 (Graph)";
        case "postgres": return "PostgreSQL サーバー";
        case "mysql": return "MySQL サーバー";
        case "box": return "Box";
        default: return repType;
    }
}
/** その接続先種別で「接続情報」として接続先側にまとめる config キー（=テーブルの編集画面からは隠す）。 */
function sharedKeysFor(kind) {
    switch (kind) {
        case "path": return ["path"];
        case "sharepoint": return ["site_url", "tenant_id", "client_id", "client_secret"];
        case "notion": return ["token"];
        case "graph365": return ["tenant_id", "client_id", "client_secret"];
        case "postgres": return ["host", "port", "database", "user", "password"];
        case "mysql": return ["host", "port", "database", "user", "password"];
        case "box": return ["client_id", "client_secret", "enterprise_id"];
        default: return [];
    }
}
// ============================================================
// テーブル単位の「開く」: 接続先とは別に、テーブル自体のWebページを開けるものだけ対応する。
// Excel/CSV/SQLite/Access 等のファイル系はファイル自体（接続先）を開く以外に意味のあるURLが
// 無いため対象外（接続先の「開く」で足りる）。
// ============================================================
const TABLE_OPENABLE_TYPES = new Set([
    "notion", "graph_entra_users", "graph_entra_groups", "graph_entra_devices",
    "graph_intune_devices", "graph_licenses", "graph_teams",
    "graph_sharepoint_list", "graph_sharepoint_library",
    "airtable", "google_sheets", "ical_calendar",
]);
/** テーブル種別・設定値から「このテーブルを開く」URLを組み立てる。組み立てられない場合は null。 */
function tableOpenUrl(type_, cfg) {
    const str = (v) => (typeof v === "string" && v.trim() ? v.trim() : undefined);
    switch (type_) {
        case "notion": {
            // Notion の URL は database_id からハイフンを除いた32桁を使う。
            const id = str(cfg.database_id);
            return id ? `https://www.notion.so/${id.replace(/-/g, "")}` : null;
        }
        case "graph_entra_users":
            return "https://entra.microsoft.com/#view/Microsoft_AAD_UsersAndTenants/UserManagementMenuBlade/~/AllUsers";
        case "graph_entra_groups":
            return "https://entra.microsoft.com/#view/Microsoft_AAD_IAM/GroupsManagementMenuBlade/~/AllGroups";
        case "graph_entra_devices":
            return "https://entra.microsoft.com/#view/Microsoft_AAD_IAM/DevicesMenuBlade/~/Devices";
        case "graph_intune_devices":
            return "https://intune.microsoft.com/#view/Microsoft_Intune_DeviceSettings/DevicesMenu/~/allDevices";
        case "graph_licenses":
            return "https://entra.microsoft.com/#view/Microsoft_AAD_IAM/LicensesMenuBlade/~/Products";
        case "graph_teams":
            return "https://admin.teams.microsoft.com/teams";
        case "graph_sharepoint_list": {
            const site = str(cfg.site_url);
            const list = str(cfg.list_name);
            if (!site || !list)
                return null;
            // 既定のリスト表示ページ。リスト名を変更している場合など、必ずしも一致しない可能性がある(ベストエフォート)。
            return `${site.replace(/\/$/, "")}/Lists/${encodeURIComponent(list)}/AllItems.aspx`;
        }
        case "graph_sharepoint_library": {
            const site = str(cfg.site_url);
            const lib = str(cfg.library_name) ?? "Shared Documents";
            if (!site)
                return null;
            return `${site.replace(/\/$/, "")}/${encodeURIComponent(lib)}/Forms/AllItems.aspx`;
        }
        case "airtable": {
            const base = str(cfg.base_id);
            const table = str(cfg.table);
            return base && table ? `https://airtable.com/${encodeURIComponent(base)}/${encodeURIComponent(table)}` : null;
        }
        case "google_sheets": {
            const id = str(cfg.spreadsheet_id);
            return id ? `https://docs.google.com/spreadsheets/d/${encodeURIComponent(id)}/edit` : null;
        }
        case "ical_calendar":
            return str(cfg.url) ?? null;
        default:
            return null;
    }
}
const dialog = $("add-dialog");
const typeSel = $("f-type");
const dynamicEl = $("f-dynamic");
const browseAreaEl = $("browse-area");
const belongsToEl = $("f-belongs-to");
const rolesRowsEl = $("roles-rows");
const rolesTableWrap = $("roles-table-wrap");
const rolesHint = $("roles-hint");
let dialogMode = "add";
let editingId = null;
let lastFieldNames = [];
/** 編集ダイアログで接続先にグルーピングされているテーブルについて、非表示にした共有(接続)フィールドのキーと元の値。
 * buildConfigFromFields() で保存時にそのまま引き継ぐ（このダイアログでは編集させない）。 */
let hiddenSharedKeys = [];
let hiddenSharedValues = {};
/** 種別が変わっても保持したい既存のフィールド役割設定（編集時の初期反映用）。 */
let currentRoleState = null;
for (const t of TYPE_DEFS) {
    const opt = document.createElement("option");
    opt.value = t.value;
    opt.textContent = t.label;
    typeSel.appendChild(opt);
}
function currentTypeDef() {
    return TYPE_DEFS.find((t) => t.value === typeSel.value);
}
function getFieldValue(f) {
    const el = document.getElementById(`fd-${f.key}`);
    if (!el)
        return undefined;
    if (f.kind === "checkbox")
        return el.checked;
    if (f.kind === "list") {
        const items = el.value.split(",").map((s) => s.trim()).filter(Boolean);
        return items.length ? items : undefined;
    }
    if (f.kind === "number") {
        const v = el.value.trim();
        if (v === "")
            return undefined; // 空欄はサーバ側デフォルト(0)に委ねる
        const n = Number(v);
        return Number.isFinite(n) ? n : undefined;
    }
    // path 系フィールドは両端のダブルクォートを除去する（要件: クォート付きパス対応）。
    if (f.key === "path")
        return normalizePath(el.value);
    return el.value.trim();
}
/** 追加時、表示名をユーザーが手動で編集したら以後はファイルパスからの自動入力で上書きしない。 */
let nameManuallyEdited = false;
function basenameOf(path) {
    const parts = path.split(/[\\/]/).filter(Boolean);
    return parts[parts.length - 1] || path;
}
/** 追加ダイアログでファイルパスを入力/選択した際、表示名が未編集ならファイル名を自動入力する。 */
function maybeAutoFillName(path) {
    if (dialogMode !== "add" || nameManuallyEdited)
        return;
    const trimmed = normalizePath(path);
    if (!trimmed)
        return;
    $("f-name").value = basenameOf(trimmed);
}
function renderTypeFields(prefill, hideKeys = []) {
    const def = currentTypeDef();
    dynamicEl.innerHTML = "";
    for (const f of def.fields) {
        if (hideKeys.includes(f.key))
            continue;
        const label = document.createElement("label");
        if (f.kind === "checkbox") {
            label.className = "check";
            const cb = document.createElement("input");
            cb.type = "checkbox";
            cb.id = `fd-${f.key}`;
            cb.checked = prefill ? Boolean(prefill[f.key]) : (f.default ?? false);
            label.append(cb, document.createTextNode(" " + f.label));
        }
        else if (f.kind === "select") {
            label.textContent = f.label;
            const sel = document.createElement("select");
            sel.id = `fd-${f.key}`;
            for (const opt of f.options ?? []) {
                const o = document.createElement("option");
                o.value = opt.value;
                o.textContent = opt.label;
                sel.appendChild(o);
            }
            if (prefill && typeof prefill[f.key] === "string")
                sel.value = prefill[f.key];
            label.appendChild(sel);
        }
        else {
            label.textContent = f.label;
            const input = document.createElement("input");
            input.type = f.kind === "password" ? "password" : f.kind === "number" ? "number" : "text";
            input.autocomplete = f.kind === "password" ? "off" : "on";
            input.id = `fd-${f.key}`;
            if (f.kind === "number" && f.min !== undefined)
                input.min = String(f.min);
            if (f.placeholder)
                input.placeholder = f.placeholder;
            if (prefill) {
                const v = prefill[f.key];
                if (Array.isArray(v))
                    input.value = v.join(", ");
                else if (v !== undefined && v !== null)
                    input.value = String(v);
            }
            if (f.key === "path") {
                input.addEventListener("input", () => maybeAutoFillName(input.value));
            }
            if (f.browse) {
                // 参照ボタン付き（ファイル/フォルダ選択ダイアログ）。
                const row = document.createElement("div");
                row.className = "path-row";
                const browseBtn = document.createElement("button");
                browseBtn.type = "button";
                browseBtn.className = "btn browse-btn";
                browseBtn.textContent = "参照…";
                browseBtn.onclick = async () => {
                    try {
                        const picked = await openDialog({
                            directory: f.browse === "folder",
                            multiple: false,
                            filters: f.browse === "file" ? f.dialogFilters : undefined,
                        });
                        if (typeof picked === "string") {
                            input.value = picked;
                            if (f.key === "path")
                                maybeAutoFillName(picked);
                        }
                    }
                    catch (e) {
                        console.error("ファイル選択に失敗:", e);
                    }
                };
                row.append(input, browseBtn);
                label.appendChild(row);
            }
            else {
                label.appendChild(input);
            }
        }
        dynamicEl.appendChild(label);
    }
    // 参照(ブラウズ)ボタンは新規追加時のみ（編集時は種別固定・接続情報も隠れているため意味がない）。
    if (dialogMode === "add") {
        renderBrowseArea(def);
    }
    else {
        browseAreaEl.innerHTML = "";
    }
    resetRolesEditor();
}
typeSel.onchange = () => renderTypeFields();
function buildConfigFromFields() {
    const def = currentTypeDef();
    // 接続先側で管理している共有(接続)フィールドは、このダイアログでは表示していないので元の値をそのまま引き継ぐ。
    const config = { ...hiddenSharedValues };
    for (const f of def.fields) {
        if (hiddenSharedKeys.includes(f.key))
            continue;
        const v = getFieldValue(f);
        if (v === undefined)
            continue;
        // 空の任意テキスト/パスワードは送らない（サーバ側の Option<...>/デフォルトに委ねる）。
        if (f.optional && (f.kind === "text" || f.kind === "password") && String(v).trim() === "")
            continue;
        config[f.key] = v;
    }
    const missingText = def.fields.some((f) => (f.kind === "text" || f.kind === "password") && !f.optional && !String(config[f.key] ?? "").trim());
    return { config, missingText };
}
// ---- フィールド役割エディタ（要件 §9-3）----
function resetRolesEditor() {
    rolesTableWrap.hidden = true;
    rolesRowsEl.innerHTML = "";
    lastFieldNames = [];
    rolesHint.textContent = "接続情報を入力してから取得してください";
}
/** 保存済みの並び順(order)を先頭に、そこに無い新規フィールドは検出順のまま末尾へ続ける。 */
function applyColumnOrder(names, order) {
    if (!order || order.length === 0)
        return names;
    const known = order.filter((n) => names.includes(n));
    const extra = names.filter((n) => !order.includes(n));
    return [...known, ...extra];
}
let rolesDragSrcName = null;
function renderRolesTable(names) {
    lastFieldNames = names;
    rolesRowsEl.innerHTML = "";
    const state = currentRoleState;
    const orderedNames = applyColumnOrder(names, state?.order);
    for (const name of orderedNames) {
        const tr = document.createElement("tr");
        tr.dataset.name = name;
        tr.draggable = true;
        tr.addEventListener("dragstart", () => { rolesDragSrcName = name; tr.classList.add("dragging"); });
        tr.addEventListener("dragend", () => tr.classList.remove("dragging"));
        tr.addEventListener("dragover", (ev) => { ev.preventDefault(); tr.classList.add("drag-over"); });
        tr.addEventListener("dragleave", () => tr.classList.remove("drag-over"));
        tr.addEventListener("drop", (ev) => {
            ev.preventDefault();
            tr.classList.remove("drag-over");
            if (rolesDragSrcName === null || rolesDragSrcName === name)
                return;
            const srcRow = rolesRowsEl.querySelector(`tr[data-name="${CSS.escape(rolesDragSrcName)}"]`);
            if (srcRow)
                rolesRowsEl.insertBefore(srcRow, tr);
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
        dispCb.checked = state ? state.display.has(name) : true;
        dispTd.appendChild(dispCb);
        const searchTd = document.createElement("td");
        const searchCb = document.createElement("input");
        searchCb.type = "checkbox";
        searchCb.className = "r-search";
        searchCb.checked = state ? state.searchable.has(name) : true;
        searchTd.appendChild(searchCb);
        const labelTd = document.createElement("td");
        const labelRadio = document.createElement("input");
        labelRadio.type = "radio";
        labelRadio.name = "r-label";
        labelRadio.value = name;
        // 役割未設定(リセット直後・新規追加時)は、暫定的に1列目を見出しに自動選択する。
        // TODO: 将来的にはAIによる推定に置き換える。
        labelRadio.checked = state ? state.label_column === name : name === orderedNames[0];
        labelTd.appendChild(labelRadio);
        const openRefTd = document.createElement("td");
        const openRefRadio = document.createElement("input");
        openRefRadio.type = "radio";
        openRefRadio.name = "r-openref";
        openRefRadio.value = name;
        openRefRadio.checked = state ? state.open_ref_column === name : false;
        openRefTd.appendChild(openRefRadio);
        const aliasTd = document.createElement("td");
        const aliasInput = document.createElement("input");
        aliasInput.type = "text";
        aliasInput.className = "r-alias";
        aliasInput.placeholder = name;
        aliasInput.value = state?.aliases[name] ?? "";
        aliasTd.appendChild(aliasInput);
        tr.append(handleTd, nameTd, dispTd, searchTd, labelTd, openRefTd, aliasTd);
        rolesRowsEl.appendChild(tr);
    }
    rolesTableWrap.hidden = names.length === 0;
    rolesHint.textContent = names.length ? `${names.length} 列を検出` : "フィールドが見つかりませんでした";
}
function collectRoleConfig() {
    const out = {};
    const displayCols = [];
    const searchableCols = [];
    const aliases = {};
    let labelCol;
    let openRefCol;
    for (const tr of Array.from(rolesRowsEl.querySelectorAll("tr"))) {
        const name = tr.dataset.name;
        if (tr.querySelector(".r-display").checked)
            displayCols.push(name);
        if (tr.querySelector(".r-search").checked)
            searchableCols.push(name);
        const labelR = tr.querySelector('input[name="r-label"]');
        if (labelR.checked)
            labelCol = name;
        const openRefR = tr.querySelector('input[name="r-openref"]');
        if (openRefR.checked)
            openRefCol = name;
        const alias = tr.querySelector(".r-alias").value.trim();
        if (alias && alias !== name)
            aliases[name] = alias;
    }
    if (lastFieldNames.length > 0) {
        out.display_columns = displayCols;
        out.searchable_columns = searchableCols;
        if (labelCol)
            out.label_column = labelCol;
        if (openRefCol)
            out.open_ref_column = openRefCol;
        if (Object.keys(aliases).length)
            out.column_aliases = aliases;
        const order = Array.from(rolesRowsEl.querySelectorAll("tr")).map((tr) => tr.dataset.name);
        out.column_order = order;
    }
    return out;
}
/** フィールド一覧を取得してロールテーブルを描画する。編集ダイアログでは開いた直後に自動実行し、
 * ボタン自体は変更後の内容を取り直す「再読込」として機能する。 */
async function fetchAndRenderFields() {
    rolesHint.textContent = "取得中…";
    try {
        let names;
        let fromCache = false;
        if (dialogMode === "edit" && editingId !== null) {
            const preview = await invoke("preview_source_fields_by_id", { id: editingId });
            names = preview.names;
            fromCache = preview.from_cache;
        }
        else {
            const { config } = buildConfigFromFields();
            names = await invoke("preview_source_fields", { type: typeSel.value, configJson: JSON.stringify(config) });
        }
        renderRolesTable(names);
        if (fromCache) {
            rolesHint.textContent += "（⚠ 接続できないためキャッシュ済みデータから復元。表示名ベースの列名です）";
        }
    }
    catch (e) {
        rolesHint.textContent = "取得失敗: " + String(e);
    }
}
$("fetch-fields").onclick = () => void fetchAndRenderFields();
/** 表示・検索設定(役割/エイリアス/並び順)をこのダイアログ上で既定値へ戻す（保存はまだされない）。 */
$("reset-fields").onclick = () => {
    currentRoleState = null;
    renderRolesTable(lastFieldNames);
};
// ---- ダイアログを開く(追加/編集) ----
function openAddDialog() {
    dialogMode = "add";
    editingId = null;
    currentRoleState = null;
    hiddenSharedKeys = [];
    hiddenSharedValues = {};
    nameManuallyEdited = false;
    belongsToEl.hidden = true;
    $("f-last-loaded").hidden = true;
    $("add-dialog-title").textContent = "テーブル追加";
    $("f-name").value = "";
    typeSel.value = "csv";
    typeSel.disabled = false;
    renderTypeFields();
    dialog.showModal();
}
$("f-name").addEventListener("input", () => { nameManuallyEdited = true; });
async function openEditDialog(id) {
    const detail = await invoke("get_source_detail", { id });
    const info = sources.find((s) => s.id === id);
    dialogMode = "edit";
    editingId = id;
    $("add-dialog-title").textContent = `編集: ${detail.name}`;
    const lastLoadedEl = $("f-last-loaded");
    if (info?.last_success_at) {
        lastLoadedEl.hidden = false;
        lastLoadedEl.textContent = `最終読み込み: ${info.last_success_at}` + (info.status === "error" ? "（直近は失敗: 前回成功時点のキャッシュを表示中）" : "");
    }
    else {
        lastLoadedEl.hidden = false;
        lastLoadedEl.textContent = "最終読み込み: まだ取得されていません";
    }
    $("f-name").value = detail.name;
    typeSel.value = detail.type_;
    typeSel.disabled = true; // 種別変更は複製で対応(接続情報の構造が変わるため編集では不可)
    // フィールド役割情報を config から抽出し、prefill から役割系キーは除いて渡す。
    const cfg = { ...detail.config };
    currentRoleState = {
        label_column: cfg.label_column,
        open_ref_column: cfg.open_ref_column,
        searchable: new Set(cfg.searchable_columns ?? []),
        display: new Set(cfg.display_columns ?? []),
        aliases: cfg.column_aliases ?? {},
        order: cfg.column_order,
    };
    delete cfg.label_column;
    delete cfg.open_ref_column;
    delete cfg.searchable_columns;
    delete cfg.display_columns;
    delete cfg.column_aliases;
    delete cfg.column_order;
    // 接続先にグルーピングされているテーブルは、共有の接続情報フィールドを
    // このダイアログから隠す（変更は接続先の右クリックメニューから行う）。保存時は元の値を引き継ぐ。
    const parentKey = info?.parent ?? "";
    hiddenSharedKeys = parentKey ? sharedKeysFor(parentKindOf(detail.type_)) : [];
    hiddenSharedValues = {};
    for (const k of hiddenSharedKeys)
        hiddenSharedValues[k] = cfg[k];
    if (parentKey && hiddenSharedKeys.length > 0) {
        belongsToEl.hidden = false;
        belongsToEl.textContent = `所属: ${parentLabel(parentKey)}（接続情報は接続先を右クリック→接続設定から変更できます）`;
    }
    else {
        belongsToEl.hidden = true;
    }
    renderTypeFields(cfg, hiddenSharedKeys);
    dialog.showModal();
    // 編集ダイアログでは開いた直後にフィールド一覧を自動取得して表示する。
    // 既存の「再読込」ボタンは、接続情報を変更した後などに手動で取り直す用途になる。
    await fetchAndRenderFields();
    if (detail.has_secret && !detail.unlocked) {
        rolesHint.textContent += "（シークレットはロック中: 保存すると変更なし扱いになります）";
    }
}
$("add").onclick = () => openAddDialog();
$("add-form").addEventListener("submit", async (e) => {
    const submitter = e.submitter;
    if (submitter && submitter.value === "cancel")
        return;
    const name = $("f-name").value.trim();
    const { config, missingText } = buildConfigFromFields();
    if (!name || missingText)
        return;
    Object.assign(config, collectRoleConfig());
    try {
        if (dialogMode === "edit" && editingId !== null) {
            await invoke("update_source", { id: editingId, name, configJson: JSON.stringify(config) });
        }
        else {
            const type_ = typeSel.value;
            if (needsMasterPassword(type_)) {
                await ensureMasterPasswordSetup();
            }
            await invoke("add_source", { type: type_, name, configJson: JSON.stringify(config) });
        }
        typeSel.disabled = false;
        await loadSources();
    }
    catch (err) {
        alert("保存に失敗: " + String(err));
    }
});
function needsMasterPassword(type_) {
    // 秘密情報（トークン/パスワード/シークレット）を扱う種別。
    return ["notion", "airtable", "postgres", "mysql", "box_files"].includes(type_) || type_.startsWith("graph");
}
// ---- 秘密フィールドは「表示」時にマスターパスワードを要求する(要件 §9-4) ----
// パスワード種別の入力(password type)はブラウザ標準でマスクされるため追加対応不要。
// ロック中の編集ダイアログでプレースホルダーが入っている場合、フィールドをクリックしたら解錠を促す。
dynamicEl.addEventListener("focusin", async (ev) => {
    const el = ev.target;
    if (el.type !== "password")
        return;
    if (el.value !== SECRET_PLACEHOLDER)
        return;
    const ok = await ensureUnlocked();
    if (ok && editingId !== null) {
        const detail = await invoke("get_source_detail", { id: editingId });
        el.value = String(detail.config[el.id.replace("fd-", "")] ?? "");
    }
});
// ============================================================
// ブラウズして追加（要件 §9-2）
// ============================================================
function renderBrowseArea(def) {
    browseAreaEl.innerHTML = "";
    if (!def.browse)
        return;
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "btn";
    switch (def.browse) {
        case "notion":
            btn.textContent = "データベース一覧から選択…";
            btn.onclick = openNotionPicker;
            break;
        case "sqlite":
            btn.textContent = "テーブル一覧から複数追加…";
            btn.onclick = () => openTablePicker("sqlite");
            break;
        case "accdb":
            btn.textContent = "テーブル一覧から複数追加…";
            btn.onclick = () => openTablePicker("accdb");
            break;
        case "excel_table":
            btn.textContent = "テーブル(ListObject)一覧から複数追加…";
            btn.onclick = () => openTablePicker("excel_table");
            break;
        case "sharepoint":
            btn.textContent = "サイト/チームから選択…";
            btn.onclick = openSharePointPicker;
            break;
    }
    browseAreaEl.appendChild(btn);
}
// ---- Notion DB ピッカー ----
async function openNotionPicker() {
    const tokenEl = document.getElementById("fd-token");
    const token = tokenEl?.value.trim();
    if (!token) {
        alert("先にインテグレーショントークンを入力してください");
        return;
    }
    const listEl = $("notion-picker-list");
    listEl.innerHTML = "<div class='picker-empty'>読み込み中…</div>";
    const dlg = $("notion-picker");
    dlg.showModal();
    try {
        const dbs = await invoke("browse_notion_databases", { token });
        listEl.innerHTML = "";
        if (dbs.length === 0) {
            listEl.innerHTML = "<div class='picker-empty'>アクセス可能なデータベースがありません（インテグレーションに共有してください）</div>";
        }
        for (const db of dbs) {
            const item = document.createElement("div");
            item.className = "picker-item";
            item.textContent = db.title || "(無題)";
            item.onclick = () => {
                document.getElementById("fd-database_id").value = db.id;
                dlg.close();
            };
            listEl.appendChild(item);
        }
    }
    catch (e) {
        setPickerMessage(listEl, `取得失敗: ${String(e)}`);
    }
}
$("notion-picker-close").onclick = () => $("notion-picker").close();
// ---- Excel/SQLite/Access テーブル一括ピッカー ----
const tablePickerDlg = $("table-picker");
const tablePickerList = $("table-picker-list");
const tablePickerCount = $("table-picker-count");
const tablePickerError = $("table-picker-error");
function updateTablePickerCount() {
    const n = tablePickerList.querySelectorAll("input:checked").length;
    tablePickerCount.textContent = `${n} 件選択中`;
}
async function openTablePicker(browse) {
    const path = normalizePath(document.getElementById("fd-path")?.value);
    if (!path) {
        alert("先にファイルパスを入力してください");
        return;
    }
    $("table-picker-title").textContent = browse === "excel_table" ? "テーブル(ListObject)一覧" : "テーブル一覧";
    tablePickerError.textContent = "";
    tablePickerList.innerHTML = "<div class='picker-empty'>読み込み中…</div>";
    tablePickerCount.textContent = "";
    tablePickerDlg.showModal();
    const cmd = browse === "sqlite" ? "browse_sqlite_tables" : browse === "accdb" ? "browse_accdb_tables" : "browse_excel_tables";
    let names;
    try {
        names = await invoke(cmd, { path });
    }
    catch (e) {
        setPickerMessage(tablePickerList, `取得失敗: ${String(e)}`);
        return;
    }
    tablePickerList.innerHTML = "";
    if (names.length === 0) {
        tablePickerList.innerHTML = "<div class='picker-empty'>見つかりませんでした</div>";
    }
    for (const name of names) {
        // 行全体を <label> にして、どこをクリックしてもチェックが切り替わるようにする。
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
    $("table-picker-add").onclick = async () => {
        const checked = Array.from(tablePickerList.querySelectorAll("input:checked"));
        if (checked.length === 0) {
            tablePickerError.textContent = "1 つ以上チェックしてください";
            return;
        }
        const type_ = browse === "excel_table" ? "excel_table" : browse;
        const failures = [];
        let added = 0;
        for (const cb of checked) {
            const tableName = cb.value;
            const config = { path, table: tableName };
            try {
                await invoke("add_source", { type: type_, name: tableName, configJson: JSON.stringify(config) });
                added++;
            }
            catch (e) {
                failures.push(`${tableName}: ${String(e)}`);
            }
        }
        await loadSources();
        if (failures.length) {
            // 失敗があればダイアログは閉じずに理由を表示する（サイレント失敗を防ぐ）。
            tablePickerError.textContent = `${added} 件追加、${failures.length} 件失敗: ${failures[0]}`;
            return;
        }
        tablePickerDlg.close();
        dialog.close();
    };
}
$("table-picker-cancel").onclick = () => tablePickerDlg.close();
$("table-picker-all").onclick = () => {
    tablePickerList.querySelectorAll("input").forEach((cb) => (cb.checked = true));
    updateTablePickerCount();
};
$("table-picker-none").onclick = () => {
    tablePickerList.querySelectorAll("input").forEach((cb) => (cb.checked = false));
    updateTablePickerCount();
};
// ---- SharePoint サイト/チーム → リスト・ライブラリ ピッカー ----
let spMode = "sites";
function graphAuthFromDialog() {
    const tenant_id = document.getElementById("fd-tenant_id")?.value.trim();
    const client_id = document.getElementById("fd-client_id")?.value.trim();
    const client_secret = document.getElementById("fd-client_secret")?.value.trim();
    if (!tenant_id || !client_id || !client_secret)
        return null;
    return { tenant_id, client_id, client_secret };
}
async function openSharePointPicker() {
    const auth = graphAuthFromDialog();
    if (!auth) {
        alert("先にテナントID/クライアントID/クライアントシークレットを入力してください");
        return;
    }
    spMode = "sites";
    $("sp-step-source").hidden = false;
    $("sp-step-resources").hidden = true;
    $("sp-picker-add").hidden = true;
    $("sp-picker").showModal();
    await loadSpSourceList(auth);
}
async function loadSpSourceList(auth) {
    const listEl = $("sp-source-list");
    listEl.innerHTML = "<div class='picker-empty'>読み込み中…</div>";
    try {
        if (spMode === "sites") {
            const sites = await invoke("browse_graph_sites", { auth });
            renderSpSourceList(sites.map((s) => ({ key: s.web_url, label: s.display_name || s.web_url })), auth, "site");
        }
        else {
            const teams = await invoke("browse_graph_teams", { auth });
            renderSpSourceList(teams.map((t) => ({ key: t.group_id, label: t.display_name })), auth, "team");
        }
    }
    catch (e) {
        setPickerMessage(listEl, `取得失敗: ${String(e)}`);
    }
}
function renderSpSourceList(items, auth, kind) {
    const listEl = $("sp-source-list");
    const filterEl = $("sp-filter");
    const draw = () => {
        const q = filterEl.value.trim().toLowerCase();
        listEl.innerHTML = "";
        const filtered = items.filter((it) => it.label.toLowerCase().includes(q));
        if (filtered.length === 0)
            listEl.innerHTML = "<div class='picker-empty'>見つかりませんでした</div>";
        for (const it of filtered) {
            const row = document.createElement("div");
            row.className = "picker-item";
            row.textContent = it.label;
            row.onclick = async () => {
                let siteUrl = it.key;
                if (kind === "team") {
                    try {
                        siteUrl = await invoke("browse_graph_team_site_url", { auth, teamGroupId: it.key });
                    }
                    catch (e) {
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
async function openSpResourceStep(auth, siteUrl, siteLabel) {
    $("sp-step-source").hidden = true;
    const stepRes = $("sp-step-resources");
    stepRes.hidden = false;
    $("sp-picker").querySelector(".sp-selected-site").textContent = `サイト: ${siteLabel} (${siteUrl})`;
    const resList = $("sp-resource-list");
    resList.innerHTML = "<div class='picker-empty'>読み込み中…</div>";
    const addBtn = $("sp-picker-add");
    addBtn.hidden = false;
    const wantKind = typeSel.value === "graph_sharepoint_library" ? "library" : "list";
    try {
        const all = await invoke("browse_graph_site_resources", { auth, siteUrl });
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
            const checked = Array.from(resList.querySelectorAll("input:checked"));
            if (checked.length === 0) {
                $("sp-picker").close();
                return;
            }
            const type_ = typeSel.value;
            for (const cb of checked) {
                const displayName = cb.value;
                const config = type_ === "graph_sharepoint_library"
                    ? { ...auth, site_url: siteUrl, library_name: displayName }
                    : { ...auth, site_url: siteUrl, list_name: displayName };
                try {
                    await invoke("add_source", { type: type_, name: displayName, configJson: JSON.stringify(config) });
                }
                catch (e) {
                    console.error(`追加失敗 (${displayName}):`, e);
                }
            }
            $("sp-picker").close();
            dialog.close();
            await loadSources();
        };
    }
    catch (e) {
        setPickerMessage(resList, `取得失敗: ${String(e)}`);
    }
}
$("sp-tab-sites").onclick = async () => {
    spMode = "sites";
    $("sp-tab-sites").classList.add("active");
    $("sp-tab-teams").classList.remove("active");
    const auth = graphAuthFromDialog();
    if (auth)
        await loadSpSourceList(auth);
};
$("sp-tab-teams").onclick = async () => {
    spMode = "teams";
    $("sp-tab-teams").classList.add("active");
    $("sp-tab-sites").classList.remove("active");
    const auth = graphAuthFromDialog();
    if (auth)
        await loadSpSourceList(auth);
};
$("sp-picker-cancel").onclick = () => $("sp-picker").close();
// ============================================================
// データソース管理ダイアログ（テーブルの削除・更新間隔 + 編集/複製へのショートカット）
// ============================================================
const manageDialog = $("manage-dialog");
const manageRows = $("manage-rows");
function renderManageRows() {
    manageRows.innerHTML = "";
    if (sources.length === 0) {
        const tr = document.createElement("tr");
        const td = document.createElement("td");
        td.colSpan = 5;
        td.className = "manage-empty";
        td.textContent = "データソースがありません。";
        tr.appendChild(td);
        manageRows.appendChild(tr);
        return;
    }
    for (const s of sources) {
        const tr = document.createElement("tr");
        const nameTd = document.createElement("td");
        nameTd.textContent = s.name;
        const typeTd = document.createElement("td");
        typeTd.textContent = s.type_;
        const countTd = document.createElement("td");
        countTd.className = "count";
        countTd.textContent = String(s.record_count);
        const intervalTd = document.createElement("td");
        const intervalInput = document.createElement("input");
        intervalInput.type = "number";
        intervalInput.className = "interval";
        intervalInput.min = "1";
        intervalInput.value = String(Math.max(1, Math.round(s.refresh_interval / 60)));
        intervalInput.onchange = async () => {
            const minutes = Math.max(1, parseInt(intervalInput.value, 10) || 1);
            intervalInput.value = String(minutes);
            await invoke("set_refresh_interval", { id: s.id, seconds: minutes * 60 });
            s.refresh_interval = minutes * 60;
        };
        intervalTd.appendChild(intervalInput);
        const actionsTd = document.createElement("td");
        actionsTd.className = "row-actions";
        const editBtn = document.createElement("button");
        editBtn.className = "btn";
        editBtn.textContent = "編集";
        editBtn.onclick = async () => { manageDialog.close(); await openEditDialog(s.id); };
        const delBtn = document.createElement("button");
        delBtn.className = "btn danger";
        delBtn.textContent = "削除";
        delBtn.onclick = async () => {
            if (!confirm(`「${s.name}」を削除しますか？（キャッシュも消えます）`))
                return;
            await invoke("delete_source", { id: s.id });
            selectedSourceIds.delete(s.id);
            await loadSources();
            renderManageRows();
        };
        actionsTd.append(editBtn, delBtn);
        tr.append(nameTd, typeTd, countTd, intervalTd, actionsTd);
        manageRows.appendChild(tr);
    }
}
$("manage").onclick = () => {
    renderManageRows();
    manageDialog.showModal();
};
$("manage-close").onclick = () => manageDialog.close();
$("manage-close-x").onclick = () => manageDialog.close();
// ============================================================
// 設定ダイアログ（自動起動 / テーマ / ズーム。要件 §9-5, REQ-UI-01, REQ-UI-02）
// ============================================================
$("settings").onclick = async () => {
    try {
        const enabled = await invoke("autostart_status");
        $("s-autostart").checked = enabled;
    }
    catch { /* noop */ }
    $("s-theme").value = currentTheme;
    updateZoomLabel();
    $("settings-dialog").showModal();
};
$("s-autostart").addEventListener("change", async (e) => {
    const checked = e.target.checked;
    try {
        await invoke("autostart_set", { enabled: checked });
    }
    catch (err) {
        alert("自動起動の設定に失敗: " + String(err));
    }
});
$("settings-close").onclick = () => $("settings-dialog").close();
let currentTheme = "dark";
function applyTheme(theme) {
    currentTheme = theme;
    document.documentElement.setAttribute("data-theme", theme);
}
async function initTheme() {
    try {
        const theme = await invoke("get_theme");
        applyTheme(theme || "dark");
    }
    catch {
        applyTheme("dark");
    }
}
$("s-theme").addEventListener("change", async (e) => {
    const theme = e.target.value;
    applyTheme(theme);
    try {
        await invoke("set_theme", { theme });
    }
    catch (err) {
        console.error("テーマ保存に失敗:", err);
    }
});
// ---- ズーム（要件 REQ-UI-01） ----
let currentZoom = 1.0;
const ZOOM_STEP = 0.1;
const ZOOM_MIN = 0.5;
const ZOOM_MAX = 3.0;
function updateZoomLabel() {
    $("s-zoom-value").textContent = `${Math.round(currentZoom * 100)}%`;
}
async function setZoom(factor) {
    currentZoom = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, Math.round(factor * 100) / 100));
    updateZoomLabel();
    try {
        await invoke("set_zoom", { factor: currentZoom });
    }
    catch (err) {
        console.error("ズーム設定に失敗:", err);
    }
}
async function initZoom() {
    try {
        currentZoom = await invoke("get_zoom");
    }
    catch {
        currentZoom = 1.0;
    }
    updateZoomLabel();
}
$("s-zoom-in").onclick = () => void setZoom(currentZoom + ZOOM_STEP);
$("s-zoom-out").onclick = () => void setZoom(currentZoom - ZOOM_STEP);
$("s-zoom-reset").onclick = () => void setZoom(1.0);
// ---- 左右分割（サイドバー幅）のドラッグリサイズ ----
const SIDEBAR_MIN_W = 120;
const SIDEBAR_MAX_W = 800;
async function initSidebarWidth() {
    let width = 240;
    try {
        width = await invoke("get_sidebar_width");
    }
    catch { /* noop */ }
    sidebarEl.style.width = width + "px";
}
sidebarResizerEl.addEventListener("mousedown", (ev) => {
    ev.preventDefault();
    const startX = ev.clientX;
    const startW = sidebarEl.getBoundingClientRect().width;
    sidebarResizerEl.classList.add("dragging");
    document.body.style.cursor = "col-resize";
    function onMove(e) {
        const w = Math.min(SIDEBAR_MAX_W, Math.max(SIDEBAR_MIN_W, startW + (e.clientX - startX)));
        sidebarEl.style.width = w + "px";
    }
    function onUp() {
        document.removeEventListener("mousemove", onMove);
        document.removeEventListener("mouseup", onUp);
        sidebarResizerEl.classList.remove("dragging");
        document.body.style.cursor = "";
        const w = Math.round(sidebarEl.getBoundingClientRect().width);
        void invoke("set_sidebar_width", { width: w }).catch(() => { });
    }
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
});
// Ctrl++ / Ctrl+- / Ctrl+0（テンキー・Shift付き記号キーの各種配列に対応）でズーム変更。
document.addEventListener("keydown", (e) => {
    if (!(e.ctrlKey || e.metaKey))
        return;
    if (["+", "=", "Add"].includes(e.key)) {
        e.preventDefault();
        void setZoom(currentZoom + ZOOM_STEP);
    }
    else if (["-", "_", "Subtract"].includes(e.key)) {
        e.preventDefault();
        void setZoom(currentZoom - ZOOM_STEP);
    }
    else if (e.key === "0") {
        e.preventDefault();
        void setZoom(1.0);
    }
});
// ============================================================
// 自動更新イベント / 起動
// ============================================================
listen("sources-updated", () => {
    void refreshSidebar();
});
// トレイ「表示」やグローバルホットキーでウィンドウが再表示された際に検索欄へフォーカス。
listen("focus-search", () => {
    searchInput.focus();
    searchInput.select();
});
(async () => {
    await initTheme();
    await initZoom();
    await initSidebarWidth();
    await loadParentLabels();
    await ensureMasterPasswordSetup();
    await loadSources();
})();

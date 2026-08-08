import { invoke } from "./ipc";
import { $ } from "./dom";
import { mountDialogAccessibility } from "./dialog-a11y";
import { mountSimplePickerDialogs } from "./simple-picker-dialogs";
import {
  appState,
  refreshSourcesList,
  replaceSourcesList,
  saveSelectedSourcesState,
} from "./state";
import {
  loadString,
  saveString,
  STORAGE_KEY_ROW_LIMIT,
} from "../shared/storage";
import { getCellUrl } from "../shared/urls";
import { mountRoleEditor } from "../features/source-editor/role-editor";
import { mountConnectionChooser } from "../features/source-editor/connection-chooser";
import { mountSourceEditorDialogFromDom, type SourceEditorDialogController } from "../features/source-editor/dialog";
import { mountParentSettingsDialogFromDom, type ParentSettingsDialogController } from "../features/source-editor/parent-dialog";
import { sharedKeysFor } from "../features/source-editor/shared-config";
import { mountCredentials } from "../features/credentials";
import { mountExportDialog } from "../features/export/dialog";
import { mountGuideViewer } from "../features/guide";
import { mountSettings } from "../features/settings";
import type { ReloadSummary, Row } from "../shared/types";
import { parentKindForType } from "../source-types/registry";
import type { ParentKind } from "../source-types/registry";
import { mountGridView } from "../features/grid/view";
import { mountGridHead } from "../features/grid/head";
import { mountColumnMapDialogFromDom } from "../features/grid/column-map";
import { mountContextMenus } from "../features/menus/context-menus";
import { parentLabel as formatParentLabel } from "../features/sources/tree";
import { mountSourcesSidebar } from "../features/sources/sidebar";
import { mountSearchControllerFromDom } from "../features/search/controller";
import { gridColumnWidth } from "../features/grid/data";
import { mountSourcePickers } from "../features/pickers";

mountSimplePickerDialogs();
mountDialogAccessibility();

const COL_W = 200; // px 既定列幅

// ==== 状態の永続化 (localStorage) ====
function saveRowLimitState(value: string) {
  saveString(STORAGE_KEY_ROW_LIMIT, value);
}
function loadRowLimitState(): string | null {
  return loadString(STORAGE_KEY_ROW_LIMIT);
}
/** ファイルパス両端のダブルクォート・空白を除去する（エクスプローラの「パスのコピー」対策）。 */
function normalizePath(v: string | undefined | null): string {
  let s = (v ?? "").trim();
  if (s.length >= 2 && s.startsWith('"') && s.endsWith('"')) s = s.slice(1, -1);
  return s.trim();
}

const searchInput = $<HTMLInputElement>("search");
const sourcesEl = $<HTMLDivElement>("sources");
const statusEl = $<HTMLDivElement>("status");
const sidebarEl = document.querySelector(".sidebar") as HTMLElement;
const sidebarResizerEl = $<HTMLDivElement>("sidebar-resizer");
const headEl = $<HTMLDivElement>("grid-head");
const viewportEl = $<HTMLDivElement>("grid-viewport");
const spacerEl = $<HTMLDivElement>("grid-spacer");
const rowsEl = $<HTMLDivElement>("grid-rows");
const resultInfo = $<HTMLDivElement>("result-info");

function reloadSummaryMessage(summary: ReloadSummary): string {
  const counts = `成功 ${summary.successful} 件、失敗 ${summary.failed} 件、取消 ${summary.cancelled} 件`;
  const firstFailure = summary.results.find((result) => result.outcome === "failed")?.error;
  return firstFailure ? `リロード完了: ${counts}。${firstFailure}` : `リロード完了: ${counts}`;
}

function parentLabel(parent: string): string {
  return formatParentLabel(parent, appState.parentLabelOverrides);
}

let renderSources: () => void;
let runSearch: () => Promise<void>;
let reloadOne: (id: number) => Promise<void>;
let renderRows: () => void;
let renderHead: () => void;
let openContextMenu: (ev: MouseEvent, id: number) => void;
let openGridContextMenu: (ev: MouseEvent) => void;
let openParentContextMenu: (ev: MouseEvent, parentKey: string) => void;
let closeAllContextMenus: () => void;
let cellText: (row: Row, col: number, rowIndex?: number) => string;
let openEditDialog: (id: number) => Promise<void>;
let openParentSettingsDialog: (parentKey: string) => Promise<void>;
let sourceEditor: SourceEditorDialogController;
let parentSettings: ParentSettingsDialogController;

function totalGridWidth(): number {
  return appState.columns.reduce(
    (sum, c) => sum + gridColumnWidth(c, appState.columnWidths, COL_W),
    0,
  );
}

const gridView = mountGridView({
  viewportEl,
  spacerEl,
  rowsEl,
  headEl,
  gridRoot: $<HTMLDivElement>("grid"),
}, {
  closeAllContextMenus: () => closeAllContextMenus(),
  openGridContextMenu: (ev) => openGridContextMenu(ev),
  totalGridWidth,
  onRowsRendered: () => {},
});
renderRows = gridView.renderRows;
cellText = gridView.cellText;

const { guideTextFor, openGuide, openGuideDoc } = mountGuideViewer();

const { openExportDialog } = mountExportDialog({
  getRowCount: () => appState.rows.length,
  getColumnCount: () => appState.columns.length,
  getColumns: () => appState.columns,
  getRows: () => appState.rows,
  cellText: (row, col, rowIndex) => cellText(row, col, rowIndex),
  setResultInfo: (text) => { resultInfo.textContent = text; },
});

const contextMenus = mountContextMenus({
  sourceMenu: $<HTMLDivElement>("ctx-menu"),
  gridMenu: $<HTMLDivElement>("grid-ctx-menu"),
  parentMenu: $<HTMLDivElement>("parent-ctx-menu"),
}, {
  parentLabel,
  reloadSummaryMessage,
  cellText: (row, col) => cellText(row, col),
  getCellUrl,
  openEditDialog: (id) => openEditDialog(id),
  openParentSettingsDialog: (key) => openParentSettingsDialog(key),
  openExportDialog: () => openExportDialog(),
  loadSources: () => loadSources(),
  renderSources: () => renderSources(),
  runSearch: () => runSearch(),
  reloadOne: (id) => reloadOne(id),
  setResultInfo: (text) => { resultInfo.textContent = text; },
  copySelected: () => gridView.copySelected(),
  copyRowAsTsv: () => gridView.copyRowAsTsv(),
  copyRowAsTsvWithHeader: () => gridView.copyRowAsTsvWithHeader(),
  copyRowAsJson: () => gridView.copyRowAsJson(),
}, tableOpenUrl);
openContextMenu = contextMenus.openContextMenu;
openGridContextMenu = contextMenus.openGridContextMenu;
openParentContextMenu = contextMenus.openParentContextMenu;
closeAllContextMenus = contextMenus.closeAllContextMenus;

const columnMap = mountColumnMapDialogFromDom({
  loadSources: () => loadSources(),
  runSearch: () => runSearch(),
});

const gridHead = mountGridHead({
  headEl,
  viewportEl,
}, {
  totalGridWidth,
  renderRows: () => renderRows(),
  refreshSidebar: () => refreshSidebar(),
  setResultInfo: (text) => { resultInfo.textContent = text; },
  openColumnSettings: (name) => columnMap.openColumnSettings(name),
  clearDropTargets,
});
renderHead = gridHead.renderHead;

const searchController = mountSearchControllerFromDom({
  onAfterSearch: () => {
    renderHead();
    renderRows();
  },
  renderSources: () => renderSources(),
});
runSearch = searchController.runSearch;

const sourcesSidebar = mountSourcesSidebar({
  sourcesEl,
  statusEl,
  sidebarEl,
  manageToggleBtn: $<HTMLButtonElement>("manage-toggle"),
  manageHeadEl: $<HTMLDivElement>("manage-head"),
  sourceFilterInput: $<HTMLInputElement>("source-filter"),
  sourceFilterClearBtn: $<HTMLButtonElement>("source-filter-clear"),
  selectAllBtn: $<HTMLButtonElement>("select-all"),
  selectNoneBtn: $<HTMLButtonElement>("select-none"),
  expandAllBtn: $<HTMLButtonElement>("expand-all"),
  collapseAllBtn: $<HTMLButtonElement>("collapse-all"),
  statusErrorsCloseBtn: $<HTMLButtonElement>("status-errors-close"),
  statusErrorsCloseXBtn: $<HTMLButtonElement>("status-errors-close-x"),
  statusErrorsDialog: $<HTMLDialogElement>("status-errors-dialog"),
  statusErrorsList: $<HTMLDivElement>("status-errors-list"),
}, {
  onSearch: () => { void runSearch(); },
  loadSources: () => loadSources(),
  refreshSidebar: () => refreshSidebar(),
  openEditDialog: (id) => openEditDialog(id),
  openParentSettingsDialog: (key) => openParentSettingsDialog(key),
  openContextMenu: (ev, id) => openContextMenu(ev, id),
  openParentContextMenu: (ev, key) => openParentContextMenu(ev, key),
  openExportDialog: () => openExportDialog(),
  setResultInfo: (text) => { resultInfo.textContent = text; },
});
renderSources = sourcesSidebar.renderSources;
reloadOne = sourcesSidebar.reloadOne;

// ============================================================
// データソース一覧（サイドバー）
// ============================================================
export async function loadSources() {
  const list = await invoke("list_sources");
  replaceSourcesList(list);
  saveSelectedSourcesState();
  renderSources();
  runSearch();
}

export async function refreshSidebar() {
  refreshSourcesList(await invoke("list_sources"));
  renderSources();
}

let currentDropTarget: HTMLElement | null = null;
let currentDropIsAfter: boolean | null = null;

function clearDropTargets() {
  if (currentDropTarget) {
    currentDropTarget.classList.remove("drop-target-before", "drop-target-after", "col-drop-target-before", "col-drop-target-after");
  } else {
    document.querySelectorAll(".drop-target-before, .drop-target-after, .col-drop-target-before, .col-drop-target-after").forEach((el) => {
      el.classList.remove("drop-target-before", "drop-target-after", "col-drop-target-before", "col-drop-target-after");
    });
  }
  currentDropTarget = null;
  currentDropIsAfter = null;
}

function handleVerticalDragOver(ev: DragEvent, el: HTMLElement): boolean {
  ev.preventDefault();
  ev.stopPropagation();
  const rect = el.getBoundingClientRect();
  const isAfter = ev.clientY >= rect.top + rect.height / 2;

  if (currentDropTarget === el && currentDropIsAfter === isAfter) {
    return isAfter;
  }

  if (currentDropTarget && currentDropTarget !== el) {
    currentDropTarget.classList.remove("drop-target-before", "drop-target-after", "col-drop-target-before", "col-drop-target-after");
  } else if (currentDropTarget === el) {
    el.classList.remove(isAfter ? "drop-target-before" : "drop-target-after");
  }

  el.classList.add(isAfter ? "drop-target-after" : "drop-target-before");
  currentDropTarget = el;
  currentDropIsAfter = isAfter;
  return isAfter;
}

/** 接続先（ツリーの親）ノードのユーザー設定表示名（parentKey -> ラベル）。未設定なら自動導出する。 */
export async function loadParentLabels() {
  try {
    const entries = await invoke("list_parent_labels");
    appState.parentLabelOverrides = new Map(entries);
  } catch { /* noop */ }
}


// ---- 行数上限の変更で再検索 ----
const savedRowLimit = loadRowLimitState();
if (savedRowLimit) {
  $<HTMLInputElement>("row-limit").value = savedRowLimit;
}
$<HTMLInputElement>("row-limit").addEventListener("change", () => {
  saveRowLimitState($<HTMLInputElement>("row-limit").value);
  runSearch();
});

// ============================================================
// 接続先と、その配下のテーブルで共有される「接続情報」フィールドの判定
// ============================================================
function parentKindOf(type_: string): ParentKind {
  return parentKindForType(type_);
}

/** テーブル種別・設定値から「このテーブルを開く」URLを組み立てる。組み立てられない場合は null。 */
function tableOpenUrl(type_: string, cfg: Record<string, unknown>): string | null {
  const str = (v: unknown): string | undefined => (typeof v === "string" && v.trim() ? v.trim() : undefined);
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
      if (!site || !list) return null;
      // 既定のリスト表示ページ。リスト名を変更している場合など、必ずしも一致しない可能性がある(ベストエフォート)。
      return `${site.replace(/\/$/, "")}/Lists/${encodeURIComponent(list)}/AllItems.aspx`;
    }
    case "graph_sharepoint_library": {
      const site = str(cfg.site_url);
      const lib = str(cfg.library_name) ?? "Shared Documents";
      if (!site) return null;
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

const rolesRowsEl = $<HTMLTableSectionElement>("roles-rows");
const rolesTableWrap = $<HTMLDivElement>("roles-table-wrap");
const rolesHint = $<HTMLDivElement>("roles-hint");

const roleEditor = mountRoleEditor({
  rowsEl: rolesRowsEl,
  tableWrap: rolesTableWrap,
  hintEl: rolesHint,
}, {
  clearDropTargets: () => clearDropTargets(),
  handleVerticalDragOver: (ev, el) => handleVerticalDragOver(ev, el),
});

const pickers = mountSourcePickers({
  getSourceEditor: () => sourceEditor,
  getParentSettings: () => parentSettings,
  normalizePath,
  loadSources,
});

const credentials = mountCredentials({
  getSourceEditor: () => sourceEditor,
  openGuide,
  openGuideDoc,
});
export async function refreshWinAuthStatus() {
  await credentials.refreshWinAuthStatus();
}
const { ensureUnlocked, wireCredentialSelector } = credentials;

sourceEditor = mountSourceEditorDialogFromDom({
  parentLabel,
  parentKindOf,
  sharedKeysFor,
  tableOpenUrl,
  loadSources,
  openParentSettingsDialog: (key) => openParentSettingsDialog(key),
  ensureUnlocked,
  guideTextFor,
  openGuide,
  normalizePath,
  roleEditor,
  pickers: {
    wireCredentialSelector,
    ...pickers,
  },
});

openEditDialog = sourceEditor.openEditDialog;

parentSettings = mountParentSettingsDialogFromDom({
  parentLabel,
  parentKindOf,
  guideTextFor,
  openGuide,
  ensureUnlocked,
  loadSources,
  runSearch,
  normalizePath,
  fieldRenderer: {
    wireCredentialSelector,
    openAirtableBasePicker: pickers.openAirtableBasePicker,
    openAirtableTablePicker: pickers.openAirtableTablePicker,
    openAirtableViewPicker: pickers.openAirtableViewPicker,
  },
});

openParentSettingsDialog = parentSettings.openParentSettingsDialog;

mountConnectionChooser({
  openAddDialog: (opts) => sourceEditor.openAddDialog(opts),
  openParentSettingsDialog: (key) => openParentSettingsDialog(key),
});

const settings = mountSettings({
  searchInput,
  sidebarEl,
  sidebarResizerEl,
  resultInfo,
  loadSources,
  reloadSummaryMessage,
});

export async function initTheme() {
  await settings.initTheme();
}

export async function initHotkey() {
  await settings.initHotkey();
}

export async function initZoom() {
  await settings.initZoom();
}

export async function initSidebarWidth() {
  await settings.initSidebarWidth();
}

/** トレイ「表示」やグローバルホットキーでウィンドウが再表示された際に検索欄へフォーカス。 */
export function focusSearchInput() {
  searchInput.focus();
  searchInput.select();
}

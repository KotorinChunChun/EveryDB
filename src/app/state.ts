import type { SortState } from "../features/grid/data";
import { includeNewSourceIds, restoreSelectedIds, selectAllIfEmpty } from "../features/sources/selection";
import { sortedGridRows } from "../features/grid/data";
import { buildGridColumns } from "../features/grid/data";
import { shouldApplySearchResponse } from "../features/search/state";
import {
  loadStringArray,
  loadNumberArray,
  saveJson,
  STORAGE_KEY_COLLAPSED_PARENTS,
  STORAGE_KEY_SELECTED_SOURCES,
} from "../shared/storage";
import type { Row, SearchResponse, SourceInfo, WinAuthStatus } from "../shared/types";

/** アプリ全体の mutable 状態。feature モジュールからは更新関数経由で触る。 */
export const appState = {
  sources: [] as SourceInfo[],
  selectedSourceIds: new Set<number>(),
  rawRows: [] as Row[],
  rows: [] as Row[],
  columns: [] as string[],
  searchSeq: 0,
  lastAppliedSeq: -1,
  selectedCell: null as { row: number; col: number } | null,
  winAuthStatus: { available: false, unlocked: false } as WinAuthStatus,
  initialLoadDone: false,
  collapsedParents: new Set<string>(loadStringArray(STORAGE_KEY_COLLAPSED_PARENTS) ?? []),
  sourceFilter: "",
  parentLabelOverrides: new Map<string, string>(),
  manageMode: false,
  widthBeforeManage: null as number | null,
  gridColumnOrder: [] as string[],
  sortState: null as SortState,
  columnWidths: {} as Record<string, number>,
  reloading: false,
  ctxTargetId: null as number | null,
  parentCtxTargetKey: null as string | null,
};

export function saveSelectedSourcesState(): void {
  saveJson(STORAGE_KEY_SELECTED_SOURCES, Array.from(appState.selectedSourceIds));
}

export function loadSelectedSourcesState(): number[] | null {
  return loadNumberArray(STORAGE_KEY_SELECTED_SOURCES);
}

export function saveCollapsedParentsState(): void {
  saveJson(STORAGE_KEY_COLLAPSED_PARENTS, Array.from(appState.collapsedParents));
}

export function setWinAuthStatus(status: WinAuthStatus): void {
  appState.winAuthStatus = status;
}

/** 初回・更新時のソース一覧と選択状態を反映する。 */
export function replaceSourcesList(next: SourceInfo[]): void {
  const oldIds = new Set(appState.sources.map((source) => source.id));
  appState.sources = next;
  if (!appState.initialLoadDone) {
    appState.initialLoadDone = true;
    const saved = loadSelectedSourcesState();
    appState.selectedSourceIds = saved !== null
      ? restoreSelectedIds(saved, next.map((source) => source.id))
      : selectAllIfEmpty(appState.selectedSourceIds, next.map((source) => source.id));
  } else {
    appState.selectedSourceIds = includeNewSourceIds(
      appState.selectedSourceIds,
      oldIds,
      next.map((source) => source.id),
    );
  }
}

export function refreshSourcesList(next: SourceInfo[]): void {
  appState.sources = next;
}

export function rebuildGridColumns(): void {
  appState.columns = buildGridColumns(appState.rawRows, appState.gridColumnOrder);
}

export function applyGridSort(): void {
  const sorted = sortedGridRows(appState.rawRows, appState.columns, appState.sortState);
  appState.rows = sorted.rows;
  appState.sortState = sorted.sortState;
}

/** 検索応答を seq 競合チェック後に grid 状態へ反映する。適用したら true。 */
export function applySearchResponse(resp: SearchResponse): boolean {
  if (!shouldApplySearchResponse(resp.seq, appState.lastAppliedSeq)) return false;
  appState.lastAppliedSeq = resp.seq;
  appState.rawRows = resp.rows;
  rebuildGridColumns();
  applyGridSort();
  appState.selectedCell = null;
  return true;
}

export function clearSelectedCell(): void {
  appState.selectedCell = null;
}

export function clearContextMenuTargets(): void {
  appState.ctxTargetId = null;
  appState.parentCtxTargetKey = null;
}

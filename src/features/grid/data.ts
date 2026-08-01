import type { Row } from "../../shared/types";

export const PINNED_GRID_COLUMNS = new Set(["#", "テーブル", "元行", "見出し"]);
export type SortState = { column: string; dir: "asc" | "desc" } | null;

const sortCollator = new Intl.Collator("ja", { numeric: true, sensitivity: "base" });

/** 検索結果のフィールドと既存の手動順序から表示列を再構築する。 */
export function buildGridColumns(rows: readonly Row[], columnOrder: readonly string[]): string[] {
  const seen = new Set<string>();
  const fields: string[] = [];
  for (const row of rows) {
    for (const field of Object.keys(row.fields)) {
      if (!seen.has(field)) {
        seen.add(field);
        fields.push(field);
      }
    }
  }
  const known = columnOrder.filter((column) => fields.includes(column));
  const extra = fields.filter((column) => !columnOrder.includes(column));
  return ["#", "テーブル", "元行", "見出し", ...known, ...extra];
}

/** グリッドの表示列名からセル文字列を得る。 */
export function gridCellText(row: Row, column: string | undefined, rowIndex?: number): string {
  if (column === "#") return rowIndex === undefined ? "" : String(rowIndex + 1);
  if (column === "テーブル") return row.source_name;
  if (column === "元行") return row.row_number === undefined || row.row_number === null ? "" : String(row.row_number);
  if (column === "見出し") return row.label ?? "";
  return column ? row.fields[column] ?? "" : "";
}

/** 次の列見出しクリック時のソート状態。昇順→降順→解除を循環する。 */
export function nextSortState(current: SortState, column: string): SortState {
  if (!current || current.column !== column) return { column, dir: "asc" };
  return current.dir === "asc" ? { column, dir: "desc" } : null;
}

/** 値を数値優先・日本語自然順で比較する。 */
export function compareCellValues(a: string, b: string): number {
  const numericA = Number(a);
  const numericB = Number(b);
  if (Number.isFinite(numericA) && Number.isFinite(numericB)) {
    return numericA < numericB ? -1 : numericA > numericB ? 1 : 0;
  }
  return sortCollator.compare(a, b);
}

/** 現在のソート状態で表示用の行を作る。対象列が消えたときはソートを解除する。 */
export function sortedGridRows(
  rawRows: readonly Row[],
  columns: readonly string[],
  sortState: SortState,
): { rows: Row[]; sortState: SortState } {
  if (!sortState || !columns.includes(sortState.column)) {
    return { rows: [...rawRows], sortState: columns.includes(sortState?.column ?? "") ? sortState : null };
  }
  const sign = sortState.dir === "asc" ? 1 : -1;
  const keyed = rawRows.map((row, index) => ({ row, index, key: gridCellText(row, sortState.column).trim() }));
  keyed.sort((left, right) => {
    if (left.key === "" || right.key === "") {
      if (left.key === right.key) return left.index - right.index;
      return left.key === "" ? 1 : -1;
    }
    const comparison = compareCellValues(left.key, right.key);
    return comparison !== 0 ? sign * comparison : left.index - right.index;
  });
  return { rows: keyed.map((entry) => entry.row), sortState };
}

export function sortHintFor(column: string, sortState: SortState): string {
  if (sortState?.column !== column) return `${column}\nクリックで昇順に並べ替え`;
  return sortState.dir === "asc" ? `${column}\nクリックで降順に並べ替え` : `${column}\nクリックで並べ替えを解除`;
}

/** 検索結果から、ソースごとに出現した表示フィールド名を1パスで収集する。 */
export function buildSourceFieldIndex(rows: readonly Row[]): Map<number, Set<string>> {
  const index = new Map<number, Set<string>>();
  for (const row of rows) {
    const fields = index.get(row.source_id) ?? new Set<string>();
    for (const field of Object.keys(row.fields)) fields.add(field);
    index.set(row.source_id, fields);
  }
  return index;
}

export function invertAliases(aliases: Record<string, string>): Map<string, string> {
  const inverted = new Map<string, string>();
  for (const [raw, display] of Object.entries(aliases)) {
    if (!inverted.has(display)) inverted.set(display, raw);
  }
  return inverted;
}

export function toRawFieldNames(displayNames: Iterable<string>, aliases: Record<string, string>): string[] {
  const inverted = invertAliases(aliases);
  return [...new Set([...displayNames].map((display) => inverted.get(display) ?? display))];
}
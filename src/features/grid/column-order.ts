/** 列見出し DnD 後の表示列順（疑似列を除く）を計算する。無効な操作なら null。 */
export function reorderDisplayColumns(
  columns: readonly string[],
  pinned: ReadonlySet<string>,
  srcCol: string,
  targetCol: string,
  insertAfter: boolean,
): string[] | null {
  const currentOrder = columns.filter((column) => !pinned.has(column));
  if (!currentOrder.includes(srcCol) || !currentOrder.includes(targetCol)) return null;
  const remaining = currentOrder.filter((column) => column !== srcCol);
  let targetIndex = remaining.indexOf(targetCol);
  if (targetIndex === -1) return null;
  if (insertAfter) targetIndex += 1;
  remaining.splice(targetIndex, 0, srcCol);
  return remaining;
}

/** グリッド表示順に合わせてソースの column_order を決める。 */
export function sourceColumnOrderForDisplay(
  sourceFields: readonly string[],
  aliases: Readonly<Record<string, string>>,
  displayOrder: readonly string[],
): string[] {
  const orderRank = new Map(displayOrder.map((name, index) => [name, index]));
  return [...sourceFields].sort((left, right) => {
    const idxLeft = orderRank.get(aliases[left] ?? left) ?? Number.MAX_SAFE_INTEGER;
    const idxRight = orderRank.get(aliases[right] ?? right) ?? Number.MAX_SAFE_INTEGER;
    return idxLeft - idxRight;
  });
}

/** 列マップ保存時の column_aliases 更新。 */
export function applyColumnAliasChange(
  aliases: Readonly<Record<string, string>>,
  currentField: string | null,
  oldDisplayName: string,
  newField: string | null,
  newDisplayName: string,
): Record<string, string> {
  const next = { ...aliases };
  if (currentField && next[currentField] === oldDisplayName) {
    delete next[currentField];
  }
  if (newField) {
    if (newField !== newDisplayName) next[newField] = newDisplayName;
    else delete next[newField];
  }
  return next;
}

/** 列マップの「表示」チェック初期値。対応ソースが無ければ null（既定のまま）。 */
export function columnMapVisibleDefault(
  rows: ReadonlyArray<{ currentField: string | null; displayColumns?: string[] }>,
): boolean | null {
  let anyMapped = false;
  let anyHidden = false;
  for (const row of rows) {
    if (!row.currentField) continue;
    anyMapped = true;
    const displayColumns = row.displayColumns;
    if (displayColumns && !displayColumns.includes(row.currentField)) anyHidden = true;
  }
  if (!anyMapped) return null;
  return !anyHidden;
}

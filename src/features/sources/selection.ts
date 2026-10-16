/** 保存済み選択から、現在も存在するソースIDだけを復元する。 */
export function restoreSelectedIds(savedIds: readonly number[] | null, validIds: Iterable<number>): Set<number> {
  const valid = new Set(validIds);
  return new Set((savedIds ?? []).filter((id) => valid.has(id)));
}

/** 初回ロード時、保存済み選択がない場合だけ全ソースを選択する。 */
export function selectAllIfEmpty(current: ReadonlySet<number>, sourceIds: Iterable<number>): Set<number> {
  if (current.size > 0) return new Set(current);
  return new Set(sourceIds);
}

/** 再読込後に新しく追加されたソースだけを既存選択へ加える。 */
export function includeNewSourceIds(
  current: ReadonlySet<number>,
  previousIds: Iterable<number>,
  currentIds: Iterable<number>,
): Set<number> {
  const result = new Set(current);
  const previous = new Set(previousIds);
  for (const id of currentIds) {
    if (!previous.has(id)) result.add(id);
  }
  return result;
}

/** 単体チェックボックスの更新。Alt指定時は対象だけを選択する。 */
export function toggleSourceSelection(
  current: ReadonlySet<number>,
  id: number,
  checked: boolean,
  altKey: boolean,
): Set<number> {
  if (altKey) return new Set([id]);
  const result = new Set(current);
  if (checked) result.add(id);
  else result.delete(id);
  return result;
}

/** 親接続先のチェックボックス更新。全選択済みなら解除、そうでなければ配下を選択する。 */
export function toggleParentSelection(
  current: ReadonlySet<number>,
  childIds: readonly number[],
  altKey: boolean,
): Set<number> {
  if (altKey) return new Set(childIds);
  const result = new Set(current);
  const allSelected = childIds.every((id) => result.has(id));
  for (const id of childIds) {
    if (allSelected) result.delete(id);
    else result.add(id);
  }
  return result;
}

/** 現在見えているソース群だけを全選択または全解除する。 */
export function setVisibleSelection(
  current: ReadonlySet<number>,
  visibleIds: Iterable<number>,
  selected: boolean,
): Set<number> {
  const result = new Set(current);
  for (const id of visibleIds) {
    if (selected) result.add(id);
    else result.delete(id);
  }
  return result;
}
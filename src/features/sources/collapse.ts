import type { SourceInfo } from "../../shared/types";

/** 親グループの折りたたみをすべて解除する。 */
export function expandAllParents(collapsed: Set<string>): void {
  collapsed.clear();
}

/** 親を持つソースの接続先キーをすべて折りたたむ。 */
export function collapseAllParents(collapsed: Set<string>, sources: readonly SourceInfo[]): void {
  for (const source of sources) {
    if (source.parent) collapsed.add(source.parent);
  }
}

/** 親グループの折りたたみ状態を反転し、折りたたみ中なら true を返す。 */
export function toggleParentCollapsed(collapsed: Set<string>, parentKey: string): boolean {
  if (collapsed.has(parentKey)) {
    collapsed.delete(parentKey);
    return false;
  }
  collapsed.add(parentKey);
  return true;
}

/** 絞り込み中は子を常に表示する（折りたたみを無視する）。 */
export function showChildrenWhileFiltered(collapsed: boolean, filterActive: boolean): boolean {
  return !collapsed || filterActive;
}

import type { SourceInfo } from "../../shared/types";

/**
 * ドラッグ対象を、ソース単体または同じ親を持つグループとして新しい位置へ移動する。
 * 実際に順番が変わらない場合は null を返す。
 */
export function reorderedSourceIds(
  sources: readonly SourceInfo[],
  draggedSourceId: number | null,
  draggedParentKey: string | null,
  targetParentKey: string | null,
  targetId: number | null,
  insertAfter: boolean,
): number[] | null {
  if (draggedSourceId === null && draggedParentKey === null) return null;

  let movingIds: number[];
  if (draggedParentKey !== null) {
    if (draggedParentKey === targetParentKey) return null;
    movingIds = sources.filter((source) => source.parent === draggedParentKey).map((source) => source.id);
  } else {
    if (draggedSourceId === null || draggedSourceId === targetId) return null;
    movingIds = [draggedSourceId];
  }
  if (movingIds.length === 0) return null;

  const currentOrder = sources.map((source) => source.id);
  const movingSet = new Set(movingIds);
  const remainingOrder = currentOrder.filter((id) => !movingSet.has(id));
  let targetIndex = remainingOrder.length;

  if (targetId !== null) {
    const index = remainingOrder.indexOf(targetId);
    if (index !== -1) targetIndex = insertAfter ? index + 1 : index;
  } else if (targetParentKey !== null) {
    const targetChildren = sources.filter((source) => source.parent === targetParentKey && !movingSet.has(source.id));
    const targetChild = insertAfter ? targetChildren[targetChildren.length - 1] : targetChildren[0];
    if (targetChild) {
      const index = remainingOrder.indexOf(targetChild.id);
      if (index !== -1) targetIndex = insertAfter ? index + 1 : index;
    }
  }

  remainingOrder.splice(targetIndex, 0, ...movingIds);
  return currentOrder.every((id, index) => id === remainingOrder[index]) ? null : remainingOrder;
}
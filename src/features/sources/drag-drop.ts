import type { SourceInfo } from "../../shared/types";
import { reorderedSourceIds } from "./reorder";

export function isVerticalDropAfter(clientY: number, rectTop: number, rectHeight: number): boolean {
  return clientY >= rectTop + rectHeight / 2;
}

export interface SourceDragSession {
  dragSrcId: number | null;
  dragParentKey: string | null;
}

export function createSourceDragSession(): SourceDragSession {
  return { dragSrcId: null, dragParentKey: null };
}

export function startSourceItemDrag(session: SourceDragSession, sourceId: number): void {
  session.dragSrcId = sourceId;
  session.dragParentKey = null;
}

export function startParentDrag(session: SourceDragSession, parentKey: string): void {
  session.dragParentKey = parentKey;
  session.dragSrcId = null;
}

export function clearSourceDragSession(session: SourceDragSession): void {
  session.dragSrcId = null;
  session.dragParentKey = null;
}

export function computeReorderIds(
  sources: readonly SourceInfo[],
  session: SourceDragSession,
  targetParentKey: string | null,
  targetId: number | null,
  insertAfter: boolean,
): number[] | null {
  return reorderedSourceIds(
    sources,
    session.dragSrcId,
    session.dragParentKey,
    targetParentKey,
    targetId,
    insertAfter,
  );
}

export type DropMarker = "drop-target-before" | "drop-target-after";

export interface SourceDropHighlight {
  target: HTMLElement | null;
  isAfter: boolean | null;
}

export function createSourceDropHighlight(): SourceDropHighlight {
  return { target: null, isAfter: null };
}

export function clearSourceDropHighlight(state: SourceDropHighlight): void {
  if (state.target) {
    state.target.classList.remove("drop-target-before", "drop-target-after");
  } else {
    document.querySelectorAll(".drop-target-before, .drop-target-after").forEach((el) => {
      el.classList.remove("drop-target-before", "drop-target-after");
    });
  }
  state.target = null;
  state.isAfter = null;
}

export function resolveSourceDropTarget(el: HTMLElement, dragParentKey: string | null): HTMLElement {
  if (dragParentKey !== null) {
    const group = el.closest<HTMLElement>(".tree-group");
    if (group) return group;
  }
  return el;
}

/** ソース一覧の dragover で挿入位置のハイライトを更新する。 */
export function updateSourceDropHighlight(
  state: SourceDropHighlight,
  targetEl: HTMLElement,
  isAfter: boolean,
): void {
  if (state.target === targetEl && state.isAfter === isAfter) return;

  if (state.target && state.target !== targetEl) {
    state.target.classList.remove("drop-target-before", "drop-target-after");
  } else if (state.target === targetEl) {
    targetEl.classList.remove(isAfter ? "drop-target-before" : "drop-target-after");
  }

  targetEl.classList.add(isAfter ? "drop-target-after" : "drop-target-before");
  state.target = targetEl;
  state.isAfter = isAfter;
}

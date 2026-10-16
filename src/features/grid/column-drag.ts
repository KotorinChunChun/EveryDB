export type ColumnDropMarker = "col-drop-target-before" | "col-drop-target-after";

export interface ColumnDropHighlight {
  target: HTMLElement | null;
  isAfter: boolean | null;
}

export function createColumnDropHighlight(): ColumnDropHighlight {
  return { target: null, isAfter: null };
}

export function clearColumnDropHighlight(state: ColumnDropHighlight): void {
  if (state.target) {
    state.target.classList.remove("col-drop-target-before", "col-drop-target-after");
  } else {
    document.querySelectorAll(".col-drop-target-before, .col-drop-target-after").forEach((el) => {
      el.classList.remove("col-drop-target-before", "col-drop-target-after");
    });
  }
  state.target = null;
  state.isAfter = null;
}

/** 列見出し dragover で挿入位置のハイライトを更新する。 */
export function updateColumnDropHighlight(
  state: ColumnDropHighlight,
  targetEl: HTMLElement,
  isAfter: boolean,
): void {
  if (state.target === targetEl && state.isAfter === isAfter) return;

  if (state.target && state.target !== targetEl) {
    state.target.classList.remove("col-drop-target-before", "col-drop-target-after");
  } else if (state.target === targetEl) {
    targetEl.classList.remove(isAfter ? "col-drop-target-before" : "col-drop-target-after");
  }

  targetEl.classList.add(isAfter ? "col-drop-target-after" : "col-drop-target-before");
  state.target = targetEl;
  state.isAfter = isAfter;
}

export function handleColumnDragOver(
  state: ColumnDropHighlight,
  ev: DragEvent,
  el: HTMLElement,
): boolean {
  ev.preventDefault();
  ev.stopPropagation();
  const rect = el.getBoundingClientRect();
  const isAfter = ev.clientX >= rect.left + rect.width / 2;
  updateColumnDropHighlight(state, el, isAfter);
  return isAfter;
}

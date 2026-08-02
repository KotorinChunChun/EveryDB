export interface CellPosition {
  row: number;
  col: number;
}

export type ArrowKey = "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight";

const ARROW_KEYS: readonly ArrowKey[] = ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"];

export function isArrowKey(key: string): key is ArrowKey {
  return (ARROW_KEYS as readonly string[]).includes(key);
}

/** 未選択時に矢印キーで最初のセルへ移動する起点。 */
export function initialCellPosition(): CellPosition {
  return { row: 0, col: 0 };
}

export function moveCellSelection(
  current: CellPosition,
  key: ArrowKey,
  rowCount: number,
  colCount: number,
): CellPosition {
  let { row, col } = current;
  switch (key) {
    case "ArrowDown":
      row = Math.min(rowCount - 1, row + 1);
      break;
    case "ArrowUp":
      row = Math.max(0, row - 1);
      break;
    case "ArrowRight":
      col = Math.min(colCount - 1, col + 1);
      break;
    case "ArrowLeft":
      col = Math.max(0, col - 1);
      break;
  }
  return { row, col };
}

export function cellSelectionSuffix(selected: CellPosition | null, row: number, col: number): string {
  return selected && selected.row === row && selected.col === col ? " sel" : "";
}

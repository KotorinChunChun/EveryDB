/** 仮想スクロールで描画すべき行番号の半開区間を計算する。 */
export function visibleRowRange(
  rowCount: number,
  scrollTop: number,
  viewportHeight: number,
  rowHeight: number,
  overscan = 5,
): { start: number; end: number } {
  const safeRowCount = Math.max(0, rowCount);
  const safeRowHeight = Math.max(1, rowHeight);
  const start = Math.max(0, Math.floor(Math.max(0, scrollTop) / safeRowHeight) - Math.max(0, overscan));
  const end = Math.min(
    safeRowCount,
    Math.ceil((Math.max(0, scrollTop) + Math.max(0, viewportHeight)) / safeRowHeight) + Math.max(0, overscan),
  );
  return { start, end: Math.max(start, end) };
}

/** 指定行が見える位置へスクロールするためのscrollTop。既に可視なら現値を返す。 */
export function scrollTopToRevealRow(
  row: number,
  currentScrollTop: number,
  viewportHeight: number,
  rowHeight: number,
): number {
  const safeRowHeight = Math.max(1, rowHeight);
  const top = Math.max(0, row) * safeRowHeight;
  const bottom = top + safeRowHeight;
  if (top < currentScrollTop) return top;
  if (bottom > currentScrollTop + Math.max(0, viewportHeight)) return Math.max(0, bottom - Math.max(0, viewportHeight));
  return currentScrollTop;
}
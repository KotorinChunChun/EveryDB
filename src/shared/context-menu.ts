/** ビューポート内に収まるようコンテキストメニューの左上座標をクランプする。 */
export function clampContextMenuPosition(
  clientX: number,
  clientY: number,
  menuWidth: number,
  menuHeight: number,
  viewportWidth: number,
  viewportHeight: number,
): { left: number; top: number } {
  const margin = 4;
  const maxLeft = Math.max(margin, viewportWidth - menuWidth - margin);
  const maxTop = Math.max(margin, viewportHeight - menuHeight - margin);
  return {
    left: Math.min(Math.max(clientX, margin), maxLeft),
    top: Math.min(Math.max(clientY, margin), maxTop),
  };
}

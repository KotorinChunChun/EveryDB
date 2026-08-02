/** 管理モード ON 時にサイドバーへ適用する幅（px）を計算する。 */
export function manageModeSidebarWidth(
  currentWidth: number,
  windowWidth: number,
  minWidth = 720,
  mainAreaMin = 320,
): number {
  const maxW = Math.max(minWidth, windowWidth - mainAreaMin);
  return Math.min(Math.max(minWidth, currentWidth), maxW);
}

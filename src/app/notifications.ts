/** 画面中央に一時メッセージを表示する。秘密値そのものは呼び出し元から渡さない。 */
export function showToast(message: string, durationMs = 1000): void {
  const el = document.createElement("div");
  el.className = "toast";
  el.textContent = message;
  document.body.appendChild(el);
  setTimeout(() => {
    el.classList.add("toast-fade-out");
    setTimeout(() => el.remove(), 400);
  }, durationMs);
}
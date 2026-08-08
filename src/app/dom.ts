import { clampContextMenuPosition } from "../shared/context-menu";

export const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

/**
 * ピッカー等の一行メッセージを差し込む。
 * メッセージには外部 API のエラー本文が混ざりうるため、innerHTML ではなく
 * textContent で描画して HTML として解釈されないようにする。
 */
export function setPickerMessage(el: HTMLElement, message: string): void {
  el.innerHTML = "";
  const d = document.createElement("div");
  d.className = "picker-empty";
  d.textContent = message;
  el.appendChild(d);
}

/** URL 入力モーダル。キャンセル時は null、OK 時は trim 済み文字列（空文字の可能性あり）。 */
export function promptUrlInput(message?: string): Promise<string | null> {
  return new Promise((resolve) => {
    const dlg = $<HTMLDialogElement>("url-auto-input-dialog");
    const hintEl = $<HTMLParagraphElement>("url-auto-input-dialog-hint");
    const inputEl = $<HTMLInputElement>("url-auto-input-dialog-input");
    const okBtn = $<HTMLButtonElement>("url-auto-input-dialog-ok");
    const cancelBtn = $<HTMLButtonElement>("url-auto-input-dialog-cancel");

    hintEl.textContent = message ?? "URL を貼り付けまたは入力してください";
    inputEl.value = "";

    const cleanup = () => {
      okBtn.onclick = null;
      cancelBtn.onclick = null;
      dlg.onclose = null;
      inputEl.onkeydown = null;
    };

    okBtn.onclick = () => {
      cleanup();
      dlg.close();
      resolve(inputEl.value.trim());
    };
    cancelBtn.onclick = () => {
      cleanup();
      dlg.close();
      resolve(null);
    };
    dlg.onclose = () => {
      cleanup();
      resolve(null);
    };

    inputEl.onkeydown = (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        cleanup();
        dlg.close();
        resolve(inputEl.value.trim());
      }
    };

    dlg.showModal();
    inputEl.focus();
  });
}

export function confirmDelete(message: string): Promise<boolean> {
  return new Promise((resolve) => {
    const dlg = $<HTMLDialogElement>("confirm-dialog");
    const msgEl = $<HTMLParagraphElement>("confirm-dialog-message");
    const okBtn = $<HTMLButtonElement>("confirm-dialog-ok");
    const cancelBtn = $<HTMLButtonElement>("confirm-dialog-cancel");

    msgEl.textContent = message;

    const cleanup = () => {
      okBtn.onclick = null;
      cancelBtn.onclick = null;
      dlg.onclose = null;
    };

    okBtn.onclick = () => {
      cleanup();
      dlg.close();
      resolve(true);
    };
    cancelBtn.onclick = () => {
      cleanup();
      dlg.close();
      resolve(false);
    };
    dlg.onclose = () => {
      cleanup();
      resolve(false);
    };

    dlg.showModal();
  });
}

/** 右クリック位置へメニューを表示し、画面端からはみ出さないよう調整する。 */
export function positionContextMenu(menu: HTMLElement, clientX: number, clientY: number): void {
  menu.hidden = false;
  const rect = menu.getBoundingClientRect();
  const { left, top } = clampContextMenuPosition(
    clientX,
    clientY,
    rect.width || menu.offsetWidth,
    rect.height || menu.offsetHeight,
    window.innerWidth,
    window.innerHeight,
  );
  menu.style.left = `${left}px`;
  menu.style.top = `${top}px`;
}

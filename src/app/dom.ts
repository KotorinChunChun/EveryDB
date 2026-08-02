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

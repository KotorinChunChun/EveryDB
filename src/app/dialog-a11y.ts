const FOCUSABLE_SELECTOR = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "[tabindex]:not([tabindex='-1'])",
].join(", ");

function focusableElements(container: HTMLElement): HTMLElement[] {
  return [...container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)]
    .filter((el) => !el.closest("[hidden]") && el.getAttribute("aria-hidden") !== "true");
}

function focusInitial(dialog: HTMLDialogElement): void {
  const field = dialog.querySelector<HTMLElement>(
    "input:not([disabled]), select:not([disabled]), textarea:not([disabled])",
  );
  const button = dialog.querySelector<HTMLElement>("button:not([disabled])");
  (field ?? button ?? dialog).focus();
}

function trapTab(dialog: HTMLDialogElement, ev: KeyboardEvent): void {
  const items = focusableElements(dialog);
  if (items.length === 0) {
    ev.preventDefault();
    dialog.focus();
    return;
  }
  const first = items[0];
  const last = items[items.length - 1];
  const active = document.activeElement;
  if (ev.shiftKey) {
    if (active === first || active === dialog) {
      ev.preventDefault();
      last.focus();
    }
  } else if (active === last) {
    ev.preventDefault();
    first.focus();
  }
}

/** 全 `<dialog>` にフォーカストラップ・閉じた後のフォーカス復帰・Esc 操作を適用する。 */
export function mountDialogAccessibility(): void {
  const restoreFocus = new WeakMap<HTMLDialogElement, HTMLElement | null>();
  const showModal = HTMLDialogElement.prototype.showModal;

  HTMLDialogElement.prototype.showModal = function showModalWithFocus(this: HTMLDialogElement) {
    restoreFocus.set(this, document.activeElement instanceof HTMLElement ? document.activeElement : null);
    showModal.call(this);
    queueMicrotask(() => focusInitial(this));
  };

  document.addEventListener("keydown", (ev) => {
    if (ev.key !== "Tab") return;
    const target = ev.target;
    if (!(target instanceof HTMLElement)) return;
    const dialog = target.closest("dialog");
    if (!(dialog instanceof HTMLDialogElement) || !dialog.open) return;
    trapTab(dialog, ev);
  });

  document.addEventListener("keydown", (ev) => {
    if (ev.key !== "Escape") return;
    const openDialogs = [...document.querySelectorAll<HTMLDialogElement>("dialog[open]")];
    const modal = openDialogs.find((dialog) => dialog.matches(":modal"));
    if (!modal) return;
    if (modal.dataset.escapeClose === "false") return;
    modal.close();
    ev.preventDefault();
  });

  document.addEventListener("close", (ev) => {
    const dialog = ev.target;
    if (!(dialog instanceof HTMLDialogElement)) return;
    const previous = restoreFocus.get(dialog);
    restoreFocus.delete(dialog);
    if (previous?.isConnected) previous.focus();
  }, true);
}

export { focusableElements, focusInitial, trapTab };

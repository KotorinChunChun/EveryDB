export interface EmptyStateElements {
  root: HTMLElement;
  grid: HTMLElement;
  sampleButton: HTMLButtonElement;
  addButton: HTMLButtonElement;
  error: HTMLElement;
}

export interface EmptyStateActions {
  addSample: () => Promise<void>;
  openConnectionChooser: () => void;
}

export function mountEmptyState(elements: EmptyStateElements, actions: EmptyStateActions) {
  let addingSample = false;

  function render(hasSources: boolean): void {
    elements.root.hidden = hasSources;
    elements.grid.hidden = !hasSources;
    if (hasSources) {
      elements.error.hidden = true;
      elements.error.textContent = "";
    }
  }

  function showError(error: unknown): void {
    elements.root.hidden = false;
    elements.grid.hidden = true;
    elements.error.textContent = `サンプルの取得に失敗しました。接続先の設定を確認して再試行してください。\n${String(error)}`;
    elements.error.hidden = false;
  }

  elements.sampleButton.addEventListener("click", async () => {
    if (addingSample) return;
    addingSample = true;
    elements.sampleButton.disabled = true;
    elements.sampleButton.setAttribute("aria-busy", "true");
    elements.error.hidden = true;
    try {
      await actions.addSample();
    } catch (error) {
      showError(error);
    } finally {
      addingSample = false;
      elements.sampleButton.disabled = false;
      elements.sampleButton.removeAttribute("aria-busy");
    }
  });
  elements.addButton.addEventListener("click", actions.openConnectionChooser);

  return { render, showError };
}

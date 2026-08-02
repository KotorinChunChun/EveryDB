export interface SearchPreset {
  id: number;
  name: string;
  query: string;
  source_ids: number[];
  created_at: string;
}

/** 検索履歴パネルを描画する。文字列は textContent で扱い、HTMLとして解釈しない。 */
export function renderHistoryPanel(
  panel: HTMLElement,
  items: readonly string[],
  onSelect: (query: string) => void,
): void {
  panel.innerHTML = "";
  if (items.length === 0) {
    appendEmptyMessage(panel, "検索履歴はありません");
    return;
  }
  for (const query of items) {
    const item = document.createElement("div");
    item.className = "history-item";
    item.textContent = query;
    item.onmousedown = (event) => {
      event.preventDefault();
      onSelect(query);
    };
    panel.appendChild(item);
  }
}

/** 検索プリセットパネルを描画する。 */
export function renderPresetPanel(
  panel: HTMLElement,
  items: readonly SearchPreset[],
  handlers: { onSelect: (preset: SearchPreset) => void; onDelete: (preset: SearchPreset) => void },
): void {
  panel.innerHTML = "";
  if (items.length === 0) {
    appendEmptyMessage(panel, "お気に入りはまだありません");
    return;
  }
  for (const preset of items) {
    const row = document.createElement("div");
    row.className = "history-item preset-item";
    const label = document.createElement("span");
    label.className = "preset-label";
    label.textContent = preset.query ? `${preset.name}（${preset.query}）` : preset.name;
    label.onmousedown = (event) => {
      event.preventDefault();
      handlers.onSelect(preset);
    };
    const deleteButton = document.createElement("button");
    deleteButton.className = "preset-del";
    deleteButton.textContent = "×";
    deleteButton.title = "削除";
    deleteButton.onmousedown = (event) => {
      event.preventDefault();
      event.stopPropagation();
      handlers.onDelete(preset);
    };
    row.append(label, deleteButton);
    panel.appendChild(row);
  }
}

function appendEmptyMessage(panel: HTMLElement, message: string): void {
  const empty = document.createElement("div");
  empty.className = "history-empty";
  empty.textContent = message;
  panel.appendChild(empty);
}
import { $ } from "../../app/dom";
import { invoke } from "../../app/ipc";
import {
  appState,
  applySearchResponse,
} from "../../app/state";
import type { SearchResponse } from "../../shared/types";
import { createSearchScheduler } from "./debounce";
import { renderHistoryPanel, renderPresetPanel, type SearchPreset } from "./panels";
import { normalizeRowLimit, presetSourceIds } from "./state";

export interface SearchControllerElements {
  searchInput: HTMLInputElement;
  historyPanel: HTMLDivElement;
  presetPanel: HTMLDivElement;
  presetSaveBtn: HTMLButtonElement;
  presetListBtn: HTMLButtonElement;
  rowLimitInput: HTMLInputElement;
  resultInfo: HTMLDivElement;
}

export interface SearchControllerActions {
  onAfterSearch: () => void;
  renderSources: () => void;
}

export interface SearchController {
  runSearch: () => Promise<void>;
  scheduleSearch: () => void;
  applyPreset: (preset: SearchPreset) => void;
}

export function mountSearchController(
  elements: SearchControllerElements,
  actions: SearchControllerActions,
): SearchController {
  const searchScheduler = createSearchScheduler(() => { void runSearch(); });

  function currentRowLimit(): number {
    return normalizeRowLimit(elements.rowLimitInput.value);
  }

  async function runSearch(): Promise<void> {
    const seq = ++appState.searchSeq;
    const query = elements.searchInput.value;
    const ids = Array.from(appState.selectedSourceIds);
    let resp: SearchResponse;
    try {
      resp = await invoke("search", { query, sourceIds: ids, seq, limit: currentRowLimit() });
    } catch (e) {
      elements.resultInfo.textContent = "検索エラー: " + String(e);
      return;
    }
    if (!applySearchResponse(resp)) return;
    actions.onAfterSearch();
    elements.resultInfo.textContent = `${appState.rows.length} 件${resp.truncated ? "（上限で打ち切り）" : ""}`;
    if (query.trim()) void invoke("add_search_history_entry", { query }).catch(() => {});
  }

  async function showHistory(): Promise<void> {
    let items: string[] = [];
    try {
      items = await invoke("list_search_history");
    } catch { /* noop */ }
    renderHistoryPanel(elements.historyPanel, items, (query) => {
      elements.searchInput.value = query;
      elements.historyPanel.hidden = true;
      void runSearch();
    });
    elements.historyPanel.hidden = false;
  }

  async function showPresetPanel(): Promise<void> {
    let items: SearchPreset[] = [];
    try {
      items = await invoke("list_search_presets");
    } catch { /* noop */ }
    renderPresetPanel(elements.presetPanel, items, {
      onSelect: (preset) => {
        applyPreset(preset);
        elements.presetPanel.hidden = true;
      },
      onDelete: (preset) => {
        void invoke("delete_search_preset", { id: preset.id }).then(() => showPresetPanel());
      },
    });
    elements.presetPanel.hidden = false;
  }

  function applyPreset(preset: SearchPreset): void {
    elements.searchInput.value = preset.query;
    appState.selectedSourceIds = presetSourceIds(preset.source_ids, appState.sources.map((s) => s.id));
    actions.renderSources();
    void runSearch();
  }

  elements.searchInput.addEventListener("focus", () => {
    if (!elements.searchInput.value) void showHistory();
  });
  elements.searchInput.addEventListener("input", () => {
    elements.historyPanel.hidden = true;
    searchScheduler.schedule();
  });
  elements.searchInput.addEventListener("blur", () => {
    setTimeout(() => { elements.historyPanel.hidden = true; }, 150);
  });

  elements.presetSaveBtn.onclick = async () => {
    const name = prompt("お気に入り名", elements.searchInput.value || "無題の検索");
    if (!name) return;
    try {
      await invoke("save_search_preset", {
        name,
        query: elements.searchInput.value,
        sourceIds: Array.from(appState.selectedSourceIds),
      });
      elements.resultInfo.textContent = `お気に入りに保存しました: ${name}`;
    } catch (e) {
      alert("保存に失敗: " + String(e));
    }
  };

  elements.presetListBtn.onclick = (ev) => {
    ev.stopPropagation();
    if (!elements.presetPanel.hidden) {
      elements.presetPanel.hidden = true;
      return;
    }
    const rect = elements.presetListBtn.getBoundingClientRect();
    elements.presetPanel.style.left = rect.left + "px";
    elements.presetPanel.style.top = rect.bottom + 4 + "px";
    void showPresetPanel();
  };

  document.addEventListener("click", (ev) => {
    if (!(ev.target as HTMLElement).closest("#preset-panel, #preset-list-btn")) {
      elements.presetPanel.hidden = true;
    }
  });

  return {
    runSearch,
    scheduleSearch: () => searchScheduler.schedule(),
    applyPreset,
  };
}

/** DOM ID から検索 UI 要素を取得して controller を組み立てる。 */
export function mountSearchControllerFromDom(
  actions: SearchControllerActions,
): SearchController {
  return mountSearchController({
    searchInput: $<HTMLInputElement>("search"),
    historyPanel: $<HTMLDivElement>("history-panel"),
    presetPanel: $<HTMLDivElement>("preset-panel"),
    presetSaveBtn: $<HTMLButtonElement>("preset-save"),
    presetListBtn: $<HTMLButtonElement>("preset-list-btn"),
    rowLimitInput: $<HTMLInputElement>("row-limit"),
    resultInfo: $<HTMLDivElement>("result-info"),
  }, actions);
}

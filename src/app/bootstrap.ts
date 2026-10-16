import { listen } from "@tauri-apps/api/event";
import {
  focusSearchInput,
  initHotkey,
  initSidebarWidth,
  initTheme,
  initZoom,
  loadParentLabels,
  loadSources,
  refreshSidebar,
  refreshWinAuthStatus,
} from "./app";

export async function bootstrap() {
  listen("sources-updated", () => {
    void refreshSidebar();
  });

  // トレイ「表示」やグローバルホットキーでウィンドウが再表示された際に検索欄へフォーカス。
  listen("focus-search", () => {
    focusSearchInput();
  });

  await initTheme();
  await initHotkey();
  await initZoom();
  await initSidebarWidth();
  await loadParentLabels();
  await refreshWinAuthStatus();
  await loadSources();
}

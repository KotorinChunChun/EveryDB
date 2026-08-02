import { $ } from "../../app/dom";
import { invoke } from "../../app/ipc";
import { appState } from "../../app/state";
import type { ReloadSummary } from "../../shared/types";

type Theme = "dark" | "light" | "night";

export interface SettingsDeps {
  searchInput: HTMLInputElement;
  sidebarEl: HTMLElement;
  sidebarResizerEl: HTMLElement;
  resultInfo: HTMLElement;
  loadSources: () => Promise<void>;
  reloadSummaryMessage: (summary: ReloadSummary) => string;
}

export interface SettingsController {
  initTheme: () => Promise<void>;
  initHotkey: () => Promise<void>;
  initZoom: () => Promise<void>;
  initSidebarWidth: () => Promise<void>;
  getCurrentTheme: () => Theme;
}

const SIDEBAR_MIN_W = 120;
const SIDEBAR_MAX_W = 800;
const ZOOM_STEP = 0.1;
const ZOOM_MIN = 0.5;
const ZOOM_MAX = 3.0;

/** 設定ダイアログ（テーマ / ホットキー / ズーム / 自動起動 / リロード）と関連初期化をマウントする。 */
export function mountSettings(deps: SettingsDeps): SettingsController {
  const { searchInput, sidebarEl, sidebarResizerEl, resultInfo, loadSources, reloadSummaryMessage } = deps;

  let currentTheme: Theme = "dark";
  let currentHotkey = "Alt+Ctrl+D";
  let currentZoom = 1.0;

  function applyTheme(theme: Theme) {
    currentTheme = theme;
    document.documentElement.setAttribute("data-theme", theme);
  }

  function updateSearchPlaceholder(hotkey: string) {
    searchInput.placeholder = `検索…（半角スペース区切りで複数語 AND / ${hotkey} で呼び出し）`;
  }

  function updateZoomLabel() {
    $<HTMLSpanElement>("s-zoom-value").textContent = `${Math.round(currentZoom * 100)}%`;
  }

  async function setZoom(factor: number) {
    currentZoom = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, Math.round(factor * 100) / 100));
    updateZoomLabel();
    try {
      await invoke("set_zoom", { factor: currentZoom });
    } catch (err) {
      console.error("ズーム設定に失敗:", err);
    }
  }

  async function applyAndSaveHotkey(newKey: string) {
    const hotkeyInput = $<HTMLInputElement>("s-hotkey");
    try {
      await invoke("set_hotkey", { hotkey: newKey });
      currentHotkey = newKey;
      if (hotkeyInput) hotkeyInput.value = currentHotkey;
      updateSearchPlaceholder(currentHotkey);
    } catch (err) {
      alert("ショートカットキーの変更に失敗しました: " + String(err));
      if (hotkeyInput) hotkeyInput.value = currentHotkey;
    }
  }

  async function reloadAllSources() {
    if (appState.reloading) return;
    appState.reloading = true;
    const btn = $<HTMLButtonElement>("s-reload");
    const status = $<HTMLSpanElement>("reload-status");
    btn.disabled = true;
    status.hidden = false;
    resultInfo.textContent = "リロード中…（完了まで操作をお待ちください）";
    try {
      const summary = await invoke<ReloadSummary>("reload_sources", { sourceIds: [] });
      resultInfo.textContent = reloadSummaryMessage(summary);
    } catch (e) {
      resultInfo.textContent = "リロード失敗: " + String(e);
    }
    await loadSources();
    btn.disabled = false;
    status.hidden = true;
    appState.reloading = false;
  }

  $<HTMLButtonElement>("settings").onclick = async () => {
    try {
      const enabled = await invoke<boolean>("autostart_status");
      $<HTMLInputElement>("s-autostart").checked = enabled;
    } catch { /* noop */ }
    $<HTMLSelectElement>("s-theme").value = currentTheme;
    const hotkeyInput = $<HTMLInputElement>("s-hotkey");
    if (hotkeyInput) hotkeyInput.value = currentHotkey;
    updateZoomLabel();
    $<HTMLDialogElement>("settings-dialog").showModal();
  };

  $<HTMLInputElement>("s-autostart").addEventListener("change", async (e) => {
    const checked = (e.target as HTMLInputElement).checked;
    try {
      await invoke("autostart_set", { enabled: checked });
    } catch (err) {
      alert("自動起動の設定に失敗: " + String(err));
    }
  });

  $<HTMLButtonElement>("settings-close").onclick = () => $<HTMLDialogElement>("settings-dialog").close();
  $<HTMLButtonElement>("s-reload").onclick = () => void reloadAllSources();

  $<HTMLSelectElement>("s-theme").addEventListener("change", async (e) => {
    const theme = (e.target as HTMLSelectElement).value as Theme;
    applyTheme(theme);
    try {
      await invoke("set_theme", { theme });
    } catch (err) {
      console.error("テーマ保存に失敗:", err);
    }
  });

  $<HTMLInputElement>("s-hotkey")?.addEventListener("keydown", (e) => {
    e.preventDefault();
    e.stopPropagation();
    if (["Control", "Alt", "Shift", "Meta", "ControlLeft", "ControlRight", "AltLeft", "AltRight", "ShiftLeft", "ShiftRight", "MetaLeft", "MetaRight"].includes(e.key)) {
      return;
    }
    const mods: string[] = [];
    if (e.altKey) mods.push("Alt");
    if (e.ctrlKey) mods.push("Ctrl");
    if (e.shiftKey) mods.push("Shift");
    if (e.metaKey) mods.push("Super");

    const isFunctionKey = /^F([1-9]|1[0-9]|2[0-4])$/.test(e.key);
    if (mods.length === 0 && !isFunctionKey) {
      return;
    }

    let key = e.key;
    if (key === " ") key = "Space";
    else if (key === "ArrowUp") key = "Up";
    else if (key === "ArrowDown") key = "Down";
    else if (key === "ArrowLeft") key = "Left";
    else if (key === "ArrowRight") key = "Right";
    else if (key.length === 1) key = key.toUpperCase();

    const hotkeyStr = [...mods, key].join("+");
    void applyAndSaveHotkey(hotkeyStr);
  });

  $<HTMLButtonElement>("s-hotkey-reset")?.addEventListener("click", () => {
    void applyAndSaveHotkey("Alt+Ctrl+D");
  });

  $<HTMLButtonElement>("s-zoom-in").onclick = () => void setZoom(currentZoom + ZOOM_STEP);
  $<HTMLButtonElement>("s-zoom-out").onclick = () => void setZoom(currentZoom - ZOOM_STEP);
  $<HTMLButtonElement>("s-zoom-reset").onclick = () => void setZoom(1.0);

  sidebarResizerEl.addEventListener("mousedown", (ev) => {
    ev.preventDefault();
    const startX = ev.clientX;
    const startW = sidebarEl.getBoundingClientRect().width;
    sidebarResizerEl.classList.add("dragging");
    document.body.style.cursor = "col-resize";
    function onMove(e: MouseEvent) {
      const maxW = appState.manageMode ? Math.max(SIDEBAR_MAX_W, window.innerWidth - 320) : SIDEBAR_MAX_W;
      const w = Math.min(maxW, Math.max(SIDEBAR_MIN_W, startW + (e.clientX - startX)));
      sidebarEl.style.width = w + "px";
    }
    function onUp() {
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
      sidebarResizerEl.classList.remove("dragging");
      document.body.style.cursor = "";
      const w = Math.round(sidebarEl.getBoundingClientRect().width);
      if (appState.manageMode) {
        appState.widthBeforeManage = appState.widthBeforeManage === null ? w : appState.widthBeforeManage;
        return;
      }
      void invoke("set_sidebar_width", { width: w }).catch(() => {});
    }
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  });

  document.addEventListener("keydown", (e) => {
    if (!(e.ctrlKey || e.metaKey)) return;

    const isZoomIn =
      e.key === "+" ||
      e.key === "Add" ||
      e.code === "NumpadAdd" ||
      e.code === "Semicolon" ||
      (e.code === "Equal" && !e.shiftKey);

    const isZoomOut =
      !isZoomIn &&
      (e.key === "-" ||
        e.key === "_" ||
        e.key === "Subtract" ||
        e.code === "Minus" ||
        e.code === "NumpadSubtract");

    if (isZoomIn) {
      e.preventDefault();
      void setZoom(currentZoom + ZOOM_STEP);
    } else if (isZoomOut) {
      e.preventDefault();
      void setZoom(currentZoom - ZOOM_STEP);
    } else if (e.key === "0" || e.code === "Numpad0" || e.code === "Digit0") {
      e.preventDefault();
      void setZoom(1.0);
    }
  });

  async function initTheme() {
    try {
      const theme = await invoke<string>("get_theme");
      applyTheme((theme as Theme) || "dark");
    } catch {
      applyTheme("dark");
    }
  }

  async function initHotkey() {
    try {
      const hk = await invoke<string>("get_hotkey");
      currentHotkey = hk || "Alt+Ctrl+D";
    } catch {
      currentHotkey = "Alt+Ctrl+D";
    }
    updateSearchPlaceholder(currentHotkey);
  }

  async function initZoom() {
    try {
      currentZoom = await invoke<number>("get_zoom");
    } catch {
      currentZoom = 1.0;
    }
    updateZoomLabel();
  }

  async function initSidebarWidth() {
    let width = 240;
    try {
      width = await invoke<number>("get_sidebar_width");
    } catch { /* noop */ }
    sidebarEl.style.width = width + "px";
  }

  return {
    initTheme,
    initHotkey,
    initZoom,
    initSidebarWidth,
    getCurrentTheme: () => currentTheme,
  };
}

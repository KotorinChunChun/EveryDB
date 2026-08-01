//! アプリ設定・自動起動・親表示名に関するTauriコマンド。

use tauri::{AppHandle, Manager, State};

use crate::app_state::AppState;
use crate::db;

fn err_to_string<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

// ---- 自動起動（要件 §9-5）----
#[tauri::command]
pub fn autostart_status(app: AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(err_to_string)
}

#[tauri::command]
pub fn autostart_set(app: AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    if enabled {
        app.autolaunch().enable().map_err(err_to_string)
    } else {
        app.autolaunch().disable().map_err(err_to_string)
    }
}

// ---- UI 設定: ズーム / テーマ（要件 REQ-UI-01 / REQ-UI-02）----
const ZOOM_META_KEY: &str = "ui_zoom";
const THEME_META_KEY: &str = "ui_theme";
const DEFAULT_ZOOM: f64 = 1.0;

/// ウィンドウのズーム倍率を設定し、次回起動用にローカル設定へ保存する。
#[tauri::command]
pub fn set_zoom(app: AppHandle, state: State<'_, AppState>, factor: f64) -> Result<(), String> {
    let factor = factor.clamp(0.5, 3.0);
    if let Some(w) = app.get_webview_window("main") {
        w.set_zoom(factor).map_err(err_to_string)?;
    }
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::set_meta(&conn, ZOOM_META_KEY, &factor.to_string()).map_err(err_to_string)
}

#[tauri::command]
pub fn get_zoom(state: State<'_, AppState>) -> Result<f64, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    Ok(db::get_meta(&conn, ZOOM_META_KEY)
        .map_err(err_to_string)?
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_ZOOM))
}

/// テーマ設定を保存する（実際の適用はフロント側で CSS の data-theme 切り替えにより行う）。
#[tauri::command]
pub fn set_theme(state: State<'_, AppState>, theme: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::set_meta(&conn, THEME_META_KEY, &theme).map_err(err_to_string)
}

#[tauri::command]
pub fn get_theme(state: State<'_, AppState>) -> Result<String, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    Ok(db::get_meta(&conn, THEME_META_KEY)
        .map_err(err_to_string)?
        .unwrap_or_else(|| "dark".to_string()))
}

// ---- UI 設定: 起動ショートカットキー（要件 §9-5）----
const HOTKEY_META_KEY: &str = "ui_hotkey";
const DEFAULT_HOTKEY: &str = "Alt+Ctrl+D";

/// 起動ショートカットキーを設定・変更し、永続化する。
#[tauri::command]
pub fn set_hotkey(
    app: AppHandle,
    state: State<'_, AppState>,
    hotkey: String,
) -> Result<(), String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let trimmed = hotkey.trim();
    if trimmed.is_empty() {
        return Err("ショートカットキーが空です".to_string());
    }
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let old = db::get_meta(&conn, HOTKEY_META_KEY)
        .map_err(err_to_string)?
        .unwrap_or_else(|| DEFAULT_HOTKEY.to_string());

    // 既存のショートカットを全て解除し、新ショートカットを登録試行する
    let _ = app.global_shortcut().unregister_all();
    if let Err(e) = app.global_shortcut().register(trimmed) {
        // 失敗時は旧キーを再登録してエラーを返す
        let _ = app.global_shortcut().register(old.as_str());
        return Err(format!("キーの登録に失敗しました: {}", e));
    }
    db::set_meta(&conn, HOTKEY_META_KEY, trimmed).map_err(err_to_string)
}

#[tauri::command]
pub fn get_hotkey(state: State<'_, AppState>) -> Result<String, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    Ok(db::get_meta(&conn, HOTKEY_META_KEY)
        .map_err(err_to_string)?
        .unwrap_or_else(|| DEFAULT_HOTKEY.to_string()))
}

// ---- UI 設定: サイドバー幅（左右分割のドラッグリサイズ）----
const SIDEBAR_WIDTH_META_KEY: &str = "ui_sidebar_width";
const DEFAULT_SIDEBAR_WIDTH: i64 = 240;

#[tauri::command]
pub fn set_sidebar_width(state: State<'_, AppState>, width: i64) -> Result<(), String> {
    let width = width.clamp(120, 800);
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::set_meta(&conn, SIDEBAR_WIDTH_META_KEY, &width.to_string()).map_err(err_to_string)
}

#[tauri::command]
pub fn get_sidebar_width(state: State<'_, AppState>) -> Result<i64, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    Ok(db::get_meta(&conn, SIDEBAR_WIDTH_META_KEY)
        .map_err(err_to_string)?
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_SIDEBAR_WIDTH))
}

// ---- 親（ファイル/サーバー）ノードの表示名オーバーライド ----
#[tauri::command]
pub fn list_parent_labels(state: State<'_, AppState>) -> Result<Vec<(String, String)>, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::list_parent_labels(&conn).map_err(err_to_string)
}

#[tauri::command]
pub fn set_parent_label(
    state: State<'_, AppState>,
    parent_key: String,
    label: String,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::set_parent_label(&conn, &parent_key, &label).map_err(err_to_string)
}

//! アプリ設定・自動起動・親表示名に関するTauriコマンド。

use tauri::{AppHandle, Manager, State};

use crate::app_state::AppState;
use crate::internal_log;
use crate::settings_service::{self, DEFAULT_HOTKEY};

fn err_to_string<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

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

#[tauri::command]
pub fn set_zoom(app: AppHandle, state: State<'_, AppState>, factor: f64) -> Result<(), String> {
    let factor = factor.clamp(0.5, 3.0);
    if let Some(w) = app.get_webview_window("main") {
        w.set_zoom(factor).map_err(err_to_string)?;
    }
    settings_service::set_zoom_meta(&state, factor).map_err(Into::into)
}

#[tauri::command]
pub fn get_zoom(state: State<'_, AppState>) -> Result<f64, String> {
    settings_service::get_zoom(&state).map_err(Into::into)
}

#[tauri::command]
pub fn set_theme(state: State<'_, AppState>, theme: String) -> Result<(), String> {
    settings_service::set_theme(&state, &theme).map_err(Into::into)
}

#[tauri::command]
pub fn get_theme(state: State<'_, AppState>) -> Result<String, String> {
    settings_service::get_theme(&state).map_err(Into::into)
}

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
    let old = settings_service::get_hotkey(&state).unwrap_or_else(|_| DEFAULT_HOTKEY.to_string());

    if let Err(error) = app.global_shortcut().unregister_all() {
        internal_log::warn(
            "hotkey",
            format!("unregister_all before change failed: {error}"),
        );
    }
    if let Err(e) = app.global_shortcut().register(trimmed) {
        if let Err(restore_error) = app.global_shortcut().register(old.as_str()) {
            internal_log::warn(
                "hotkey",
                format!("restore previous hotkey after failed register failed: {restore_error}"),
            );
        }
        return Err(format!("キーの登録に失敗しました: {}", e));
    }
    settings_service::set_hotkey_meta(&state, trimmed).map_err(Into::into)
}

#[tauri::command]
pub fn get_hotkey(state: State<'_, AppState>) -> Result<String, String> {
    settings_service::get_hotkey(&state).map_err(Into::into)
}

#[tauri::command]
pub fn set_sidebar_width(state: State<'_, AppState>, width: i64) -> Result<(), String> {
    settings_service::set_sidebar_width(&state, width).map_err(Into::into)
}

#[tauri::command]
pub fn get_sidebar_width(state: State<'_, AppState>) -> Result<i64, String> {
    settings_service::get_sidebar_width(&state).map_err(Into::into)
}

#[tauri::command]
pub fn list_parent_labels(state: State<'_, AppState>) -> Result<Vec<(String, String)>, String> {
    settings_service::list_parent_labels(&state).map_err(Into::into)
}

#[tauri::command]
pub fn set_parent_label(
    state: State<'_, AppState>,
    parent_key: String,
    label: String,
) -> Result<(), String> {
    settings_service::set_parent_label(&state, &parent_key, &label).map_err(Into::into)
}

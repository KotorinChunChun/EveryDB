//! UI 永続化設定（meta / 親ラベル）。Tauri ウィンドウ操作は command 層に残す。

use crate::app_state::AppState;
use crate::db;
use crate::service_error::{lock_db, ServiceError, ServiceResult};

pub const ZOOM_META_KEY: &str = "ui_zoom";
pub const THEME_META_KEY: &str = "ui_theme";
pub const HOTKEY_META_KEY: &str = "ui_hotkey";
pub const SIDEBAR_WIDTH_META_KEY: &str = "ui_sidebar_width";
pub const DEFAULT_ZOOM: f64 = 1.0;
pub const DEFAULT_HOTKEY: &str = "Alt+Ctrl+D";
pub const DEFAULT_SIDEBAR_WIDTH: i64 = 240;

pub fn get_zoom(state: &AppState) -> ServiceResult<f64> {
    let conn = lock_db(state)?;
    Ok(db::get_meta(&conn, ZOOM_META_KEY)
        .map_err(|e| ServiceError::external("ズーム設定の読み込みに失敗しました", e))?
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_ZOOM))
}

pub fn set_zoom_meta(state: &AppState, factor: f64) -> ServiceResult<()> {
    let factor = factor.clamp(0.5, 3.0);
    let conn = lock_db(state)?;
    db::set_meta(&conn, ZOOM_META_KEY, &factor.to_string())
        .map_err(|e| ServiceError::external("ズーム設定の保存に失敗しました", e))
}

pub fn get_theme(state: &AppState) -> ServiceResult<String> {
    let conn = lock_db(state)?;
    Ok(db::get_meta(&conn, THEME_META_KEY)
        .map_err(|e| ServiceError::external("テーマ設定の読み込みに失敗しました", e))?
        .unwrap_or_else(|| "dark".to_string()))
}

pub fn set_theme(state: &AppState, theme: &str) -> ServiceResult<()> {
    let conn = lock_db(state)?;
    db::set_meta(&conn, THEME_META_KEY, theme)
        .map_err(|e| ServiceError::external("テーマ設定の保存に失敗しました", e))
}

pub fn get_hotkey(state: &AppState) -> ServiceResult<String> {
    let conn = lock_db(state)?;
    Ok(db::get_meta(&conn, HOTKEY_META_KEY)
        .map_err(|e| ServiceError::external("ショートカット設定の読み込みに失敗しました", e))?
        .unwrap_or_else(|| DEFAULT_HOTKEY.to_string()))
}

pub fn set_hotkey_meta(state: &AppState, hotkey: &str) -> ServiceResult<()> {
    let trimmed = hotkey.trim();
    if trimmed.is_empty() {
        return Err(ServiceError::validation("ショートカットキーが空です"));
    }
    let conn = lock_db(state)?;
    db::set_meta(&conn, HOTKEY_META_KEY, trimmed)
        .map_err(|e| ServiceError::external("ショートカット設定の保存に失敗しました", e))
}

pub fn get_sidebar_width(state: &AppState) -> ServiceResult<i64> {
    let conn = lock_db(state)?;
    Ok(db::get_meta(&conn, SIDEBAR_WIDTH_META_KEY)
        .map_err(|e| ServiceError::external("サイドバー幅の読み込みに失敗しました", e))?
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_SIDEBAR_WIDTH))
}

pub fn set_sidebar_width(state: &AppState, width: i64) -> ServiceResult<()> {
    let width = width.clamp(120, 800);
    let conn = lock_db(state)?;
    db::set_meta(&conn, SIDEBAR_WIDTH_META_KEY, &width.to_string())
        .map_err(|e| ServiceError::external("サイドバー幅の保存に失敗しました", e))
}

pub fn list_parent_labels(state: &AppState) -> ServiceResult<Vec<(String, String)>> {
    let conn = lock_db(state)?;
    db::list_parent_labels(&conn)
        .map_err(|e| ServiceError::external("親ラベル一覧の取得に失敗しました", e))
}

pub fn set_parent_label(state: &AppState, parent_key: &str, label: &str) -> ServiceResult<()> {
    let conn = lock_db(state)?;
    db::set_parent_label(&conn, parent_key, label)
        .map_err(|e| ServiceError::external("親ラベルの保存に失敗しました", e))
}

#[cfg(test)]
mod tests {
    use super::{get_hotkey, get_theme, set_hotkey_meta, set_theme};
    use crate::app_state::AppState;
    use crate::db;

    #[test]
    fn persists_theme_and_hotkey_meta() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        let state = AppState::new(conn);
        set_theme(&state, "night").unwrap();
        set_hotkey_meta(&state, "Alt+Shift+E").unwrap();
        assert_eq!(get_theme(&state).unwrap(), "night");
        assert_eq!(get_hotkey(&state).unwrap(), "Alt+Shift+E");
    }
}

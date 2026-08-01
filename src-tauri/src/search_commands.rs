//! 検索、検索履歴、検索プリセットに関するTauriコマンド。

use tauri::State;

use crate::app_state::AppState;
use crate::db;
use crate::model::SearchResponse;

fn err_to_string<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

#[tauri::command]
pub fn search(
    state: State<'_, AppState>,
    query: String,
    source_ids: Vec<i64>,
    seq: i64,
    limit: Option<i64>,
) -> Result<SearchResponse, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let limit = limit.unwrap_or(db::DEFAULT_ROW_LIMIT);
    let (rows, truncated) = db::search(&conn, &query, &source_ids, limit).map_err(err_to_string)?;
    Ok(SearchResponse {
        seq,
        rows,
        truncated,
    })
}

/// 検索履歴の保存・再利用（要件 §9-5）。
#[tauri::command]
pub fn add_search_history_entry(state: State<'_, AppState>, query: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::add_search_history(&conn, &query).map_err(err_to_string)
}

#[tauri::command]
pub fn list_search_history(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::list_search_history(&conn, 15).map_err(err_to_string)
}

// ---- 検索プリセット/お気に入り（要件 REQ-BM-01）----
#[tauri::command]
pub fn save_search_preset(
    state: State<'_, AppState>,
    name: String,
    query: String,
    source_ids: Vec<i64>,
) -> Result<i64, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::save_search_preset(&conn, &name, &query, &source_ids).map_err(err_to_string)
}

#[tauri::command]
pub fn list_search_presets(
    state: State<'_, AppState>,
) -> Result<Vec<crate::model::SearchPreset>, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::list_search_presets(&conn).map_err(err_to_string)
}

#[tauri::command]
pub fn delete_search_preset(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::delete_search_preset(&conn, id).map_err(err_to_string)
}

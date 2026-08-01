//! 検索、検索履歴、検索プリセットに関するTauriコマンド。

use tauri::State;

use crate::app_state::AppState;
use crate::model::{SearchPreset, SearchResponse};
use crate::search_service;

#[tauri::command]
pub fn search(
    state: State<'_, AppState>,
    query: String,
    source_ids: Vec<i64>,
    seq: i64,
    limit: Option<i64>,
) -> Result<SearchResponse, String> {
    search_service::search(&state, &query, &source_ids, seq, limit)
}

/// 検索履歴の保存・再利用（要件 §9-5）。
#[tauri::command]
pub fn add_search_history_entry(state: State<'_, AppState>, query: String) -> Result<(), String> {
    search_service::add_history(&state, &query)
}

#[tauri::command]
pub fn list_search_history(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    search_service::list_history(&state)
}

// ---- 検索プリセット/お気に入り（要件 REQ-BM-01）----
#[tauri::command]
pub fn save_search_preset(
    state: State<'_, AppState>,
    name: String,
    query: String,
    source_ids: Vec<i64>,
) -> Result<i64, String> {
    search_service::save_preset(&state, &name, &query, &source_ids)
}

#[tauri::command]
pub fn list_search_presets(state: State<'_, AppState>) -> Result<Vec<SearchPreset>, String> {
    search_service::list_presets(&state)
}

#[tauri::command]
pub fn delete_search_preset(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    search_service::delete_preset(&state, id)
}

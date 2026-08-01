//! データソース操作に関するTauriコマンド。

pub use crate::app_state::AppState;

use tauri::State;

use crate::model::SourceInfo;
use crate::refresh::ReloadSummary;
use crate::source_service::{self, FieldPreview};

#[tauri::command]
pub fn list_sources(state: State<'_, AppState>) -> Result<Vec<SourceInfo>, String> {
    source_service::list_sources(&state)
}

// 保存・更新・兄弟からの追加・削除は初回取得で外部I/Oを伴うため、Tauriの非同期実行を維持する。
#[tauri::command]
pub async fn add_source(
    state: State<'_, AppState>,
    type_: String,
    name: String,
    config_json: String,
) -> Result<i64, String> {
    source_service::add_source(&state, type_, name, config_json)
}

#[tauri::command]
pub async fn add_source_from_sibling(
    state: State<'_, AppState>,
    sibling_id: i64,
    type_: String,
    name: String,
    config_json: String,
    shared_keys: Vec<String>,
) -> Result<i64, String> {
    source_service::add_source_from_sibling(
        &state,
        sibling_id,
        type_,
        name,
        config_json,
        shared_keys,
    )
}

#[tauri::command]
pub async fn delete_source(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    source_service::delete_source(&state, id)
}

#[tauri::command]
pub fn get_source_detail(state: State<'_, AppState>, id: i64) -> Result<serde_json::Value, String> {
    source_service::get_source_detail(&state, id)
}

#[tauri::command]
pub async fn update_source(
    state: State<'_, AppState>,
    id: i64,
    name: String,
    config_json: String,
) -> Result<(), String> {
    source_service::update_source(&state, id, name, config_json)
}

#[tauri::command]
pub fn duplicate_source(
    state: State<'_, AppState>,
    id: i64,
    new_name: String,
) -> Result<i64, String> {
    source_service::duplicate_source(&state, id, new_name)
}

#[tauri::command]
pub fn reorder_sources(state: State<'_, AppState>, ordered_ids: Vec<i64>) -> Result<(), String> {
    source_service::reorder_sources(&state, ordered_ids)
}

#[tauri::command]
pub fn preview_source_fields(
    state: State<'_, AppState>,
    type_: String,
    config_json: String,
) -> Result<Vec<String>, String> {
    source_service::preview_source_fields(&state, type_, config_json)
}

#[tauri::command]
pub fn preview_source_fields_by_id(
    state: State<'_, AppState>,
    id: i64,
) -> Result<FieldPreview, String> {
    source_service::preview_source_fields_by_id(&state, id)
}

#[tauri::command]
pub fn set_refresh_interval(
    state: State<'_, AppState>,
    id: i64,
    seconds: i64,
) -> Result<(), String> {
    source_service::set_refresh_interval(&state, id, seconds)
}

#[tauri::command]
pub fn reload_sources(
    state: State<'_, AppState>,
    source_ids: Vec<i64>,
) -> Result<ReloadSummary, String> {
    source_service::reload_sources(&state, source_ids)
}

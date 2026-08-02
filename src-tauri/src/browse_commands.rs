use tauri::State;

use crate::app_state::AppState;
use crate::browse;
use crate::browse_service;

#[tauri::command]
pub fn browse_notion_databases(token: String) -> Result<Vec<browse::NotionDbEntry>, String> {
    browse_service::browse_notion_databases(&token).map_err(Into::into)
}

#[tauri::command]
pub fn browse_notion_databases_for_source(
    state: State<'_, AppState>,
    id: i64,
    overrides_json: String,
) -> Result<Vec<browse::NotionDbEntry>, String> {
    browse_service::browse_notion_databases_for_source(&state, id, &overrides_json)
        .map_err(Into::into)
}

#[tauri::command]
pub fn browse_airtable_bases(token: String) -> Result<Vec<browse::AirtableBaseEntry>, String> {
    browse_service::browse_airtable_bases(&token).map_err(Into::into)
}

#[tauri::command]
pub fn browse_airtable_bases_for_source(
    state: State<'_, AppState>,
    id: i64,
    overrides_json: String,
) -> Result<Vec<browse::AirtableBaseEntry>, String> {
    browse_service::browse_airtable_bases_for_source(&state, id, &overrides_json)
        .map_err(Into::into)
}

#[tauri::command]
pub fn browse_airtable_tables(
    token: String,
    base_id: String,
) -> Result<Vec<browse::AirtableTableEntry>, String> {
    browse_service::browse_airtable_tables(&token, &base_id).map_err(Into::into)
}

#[tauri::command]
pub fn browse_airtable_tables_for_source(
    state: State<'_, AppState>,
    id: i64,
    overrides_json: String,
    base_id: Option<String>,
) -> Result<Vec<browse::AirtableTableEntry>, String> {
    browse_service::browse_airtable_tables_for_source(&state, id, &overrides_json, base_id)
        .map_err(Into::into)
}

#[tauri::command]
pub fn browse_graph_site_resources_for_source(
    state: State<'_, AppState>,
    id: i64,
    overrides_json: String,
) -> Result<Vec<browse::SiteResourceEntry>, String> {
    browse_service::browse_graph_site_resources_for_source(&state, id, &overrides_json)
        .map_err(Into::into)
}

#[tauri::command]
pub fn browse_graph_sites(auth: browse::GraphAuthArgs) -> Result<Vec<browse::SiteEntry>, String> {
    browse_service::browse_graph_sites(&auth).map_err(Into::into)
}

#[tauri::command]
pub fn browse_graph_teams(auth: browse::GraphAuthArgs) -> Result<Vec<browse::TeamEntry>, String> {
    browse_service::browse_graph_teams(&auth).map_err(Into::into)
}

#[tauri::command]
pub fn browse_graph_team_site_url(
    auth: browse::GraphAuthArgs,
    team_group_id: String,
) -> Result<String, String> {
    browse_service::browse_graph_team_site_url(&auth, &team_group_id).map_err(Into::into)
}

#[tauri::command]
pub fn browse_graph_site_resources(
    auth: browse::GraphAuthArgs,
    site_url: String,
) -> Result<Vec<browse::SiteResourceEntry>, String> {
    browse_service::browse_graph_site_resources(&auth, &site_url).map_err(Into::into)
}

#[tauri::command]
pub fn browse_excel_sheets(path: String) -> Result<Vec<String>, String> {
    browse_service::browse_excel_sheets(&path).map_err(Into::into)
}

#[tauri::command]
pub fn browse_excel_tables(path: String) -> Result<Vec<String>, String> {
    browse_service::browse_excel_tables(&path).map_err(Into::into)
}

#[tauri::command]
pub fn browse_excel_grid(
    path: String,
    sheet: String,
    rows: usize,
    cols: usize,
    mode: Option<String>,
) -> Result<browse::ExcelGridPreview, String> {
    let mode_str = mode.as_deref().unwrap_or("top");
    browse_service::browse_excel_grid(&path, &sheet, rows, cols, mode_str).map_err(Into::into)
}

#[tauri::command]
pub fn browse_sqlite_tables(path: String) -> Result<Vec<String>, String> {
    browse_service::browse_sqlite_tables(&path).map_err(Into::into)
}

#[tauri::command]
pub fn browse_accdb_tables(path: String) -> Result<Vec<String>, String> {
    browse_service::browse_accdb_tables(&path).map_err(Into::into)
}

#[tauri::command]
pub fn browse_browser_profiles(
    browser: String,
    target: String,
) -> Result<Vec<browse::BrowserProfileEntry>, String> {
    browse_service::browse_browser_profiles(&browser, &target).map_err(Into::into)
}

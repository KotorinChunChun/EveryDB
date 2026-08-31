use std::path::Path;

use tauri::{AppHandle, Manager, State};

use crate::app_state::AppState;
use crate::backup_service::{self, BackupInfo};

#[tauri::command]
pub async fn export_database_backup(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<BackupInfo, String> {
    let destination = Path::new(&path);
    let live = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("cache.db");
    if destination == live {
        return Err("動作中のcache.dbはバックアップ先に指定できません".into());
    }
    backup_service::export_backup(&state, destination).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn stage_database_restore(app: AppHandle, path: String) -> Result<BackupInfo, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    backup_service::stage_restore(&dir, Path::new(&path)).map_err(|error| error.to_string())
}

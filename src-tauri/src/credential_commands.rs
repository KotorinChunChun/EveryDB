use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tauri::{AppHandle, Manager, State};

use crate::app_state::AppState;
use crate::credential_service;

fn err_to_string<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

#[tauri::command]
pub async fn winauth_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    Ok(
        serde_json::json!({ "available": crate::winauth::is_available(), "unlocked": state.is_unlocked() }),
    )
}

#[tauri::command]
pub async fn winauth_verify(app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    if !crate::winauth::is_available() {
        state.unlock();
        return Ok(true);
    }
    #[cfg(windows)]
    let hwnd = app
        .get_webview_window("main")
        .ok_or("メインウィンドウが見つかりません")?
        .hwnd()
        .map_err(err_to_string)?;
    #[cfg(not(windows))]
    let hwnd = {
        let _ = &app;
        ()
    };
    let ok = crate::winauth::verify(hwnd, "EveryDB: 保存済みのトークンを表示します")
        .map_err(err_to_string)?;
    if ok {
        state.unlock();
    }
    Ok(ok)
}

#[tauri::command]
pub fn list_credentials(
    state: State<'_, AppState>,
    scope: String,
) -> Result<Vec<crate::credentials::CredentialSummary>, String> {
    credential_service::list_credentials(&state, &scope).map_err(Into::into)
}

#[tauri::command]
pub fn add_credential(
    state: State<'_, AppState>,
    scope: String,
    label: String,
    fields_json: String,
) -> Result<i64, String> {
    credential_service::add_credential(&state, &scope, &label, &fields_json).map_err(Into::into)
}

#[tauri::command]
pub fn update_credential(
    state: State<'_, AppState>,
    id: i64,
    label: String,
    fields_json: String,
) -> Result<(), String> {
    credential_service::update_credential(&state, id, &label, &fields_json).map_err(Into::into)
}

#[tauri::command]
pub fn delete_credential(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    credential_service::delete_credential(&state, id).map_err(Into::into)
}

#[tauri::command]
pub fn credential_usage(
    state: State<'_, AppState>,
    id: i64,
) -> Result<Vec<crate::credentials::CredentialUsage>, String> {
    credential_service::credential_usage(&state, id).map_err(Into::into)
}

#[tauri::command]
pub fn credential_public_fields(
    state: State<'_, AppState>,
    id: i64,
) -> Result<serde_json::Value, String> {
    credential_service::credential_public_fields(&state, id).map_err(Into::into)
}

#[tauri::command]
pub fn credential_secret_value(state: State<'_, AppState>, id: i64) -> Result<String, String> {
    credential_service::credential_secret_value(&state, id).map_err(Into::into)
}

#[tauri::command]
pub async fn box_oauth_authorize(
    app: AppHandle,
    state: State<'_, AppState>,
    client_id: String,
    client_secret: String,
) -> Result<serde_json::Value, String> {
    let client_id = client_id.trim().to_string();
    let client_secret = client_secret.trim().to_string();
    if client_id.is_empty() || client_secret.is_empty() {
        return Err("クライアントIDとクライアントシークレットを先に入力してください".to_string());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut slot = state.box_oauth_cancel.lock().map_err(|_| "lock poisoned")?;
        *slot = Some(cancel.clone());
    }
    let handle = app.clone();
    let cancel_for_task = cancel.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        crate::box_oauth::authorize_interactive(
            &handle,
            &client_id,
            &client_secret,
            &cancel_for_task,
        )
    })
    .await
    .map_err(|e| format!("内部エラー: {e}"))?;
    if let Ok(mut slot) = state.box_oauth_cancel.lock() {
        *slot = None;
    }
    let (_access_token, refresh_token) = result.map_err(err_to_string)?;
    if refresh_token.is_empty() {
        return Err("Box からリフレッシュトークンが返りませんでした。アプリの認証方式が「ユーザー認証(標準 OAuth 2.0)」になっているか確認してください".to_string());
    }
    Ok(serde_json::json!({ "refresh_token": refresh_token }))
}

#[tauri::command]
pub fn box_oauth_cancel(state: State<'_, AppState>) -> Result<(), String> {
    credential_service::request_box_oauth_cancel(&state).map_err(Into::into)
}

#[tauri::command]
pub async fn graph_oauth_authorize(
    app: AppHandle,
    state: State<'_, AppState>,
    tenant_id: String,
    client_id: String,
    capabilities: Vec<String>,
) -> Result<serde_json::Value, String> {
    let tenant_id = tenant_id.trim().to_string();
    let client_id = client_id.trim().to_string();
    if tenant_id.is_empty() || client_id.is_empty() {
        return Err("テナントIDとクライアントIDを先に入力してください".to_string());
    }
    crate::graph_oauth::delegated_scopes(&capabilities).map_err(err_to_string)?;
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut slot = state
            .graph_oauth_cancel
            .lock()
            .map_err(|_| "lock poisoned")?;
        *slot = Some(cancel.clone());
    }
    let handle = app.clone();
    let cancel_for_task = cancel.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        crate::graph_oauth::authorize_interactive(
            &handle,
            &tenant_id,
            &client_id,
            &capabilities,
            &cancel_for_task,
        )
    })
    .await
    .map_err(|e| format!("内部エラー: {e}"))?;
    if let Ok(mut slot) = state.graph_oauth_cancel.lock() {
        *slot = None;
    }
    let (_access_token, refresh_token) = result.map_err(err_to_string)?;
    if refresh_token.is_empty() {
        return Err(
            "Microsoft からリフレッシュトークンが返りませんでした。委任アクセス許可に offline_access が含まれているか確認してください"
                .to_string(),
        );
    }
    Ok(serde_json::json!({ "refresh_token": refresh_token }))
}

#[tauri::command]
pub fn graph_oauth_cancel(state: State<'_, AppState>) -> Result<(), String> {
    credential_service::request_graph_oauth_cancel(&state).map_err(Into::into)
}

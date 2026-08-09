//! 保存済みトークン（credential）の業務処理。Tauri 型へ依存しない。

use serde_json::Value;

use crate::app_state::AppState;
use crate::credentials::{
    self, CredentialPermissionInspection, CredentialSummary, CredentialUsage,
};
use crate::service_error::{lock_db, ServiceError, ServiceResult};

pub fn list_credentials(state: &AppState, scope: &str) -> ServiceResult<Vec<CredentialSummary>> {
    let conn = lock_db(state)?;
    credentials::list_credentials(&conn, scope)
        .map_err(|e| ServiceError::external("一覧取得に失敗しました", e))
}

pub fn add_credential(
    state: &AppState,
    scope: &str,
    label: &str,
    fields_json: &str,
) -> ServiceResult<i64> {
    let conn = lock_db(state)?;
    credentials::add_credential(&conn, scope, label, fields_json).map_err(|error| {
        if error.to_string().starts_with("Graph資格情報") {
            ServiceError::validation(error.to_string())
        } else {
            ServiceError::external("トークンの追加に失敗しました", error)
        }
    })
}

pub fn update_credential(
    state: &AppState,
    id: i64,
    label: &str,
    fields_json: &str,
) -> ServiceResult<()> {
    let conn = lock_db(state)?;
    credentials::update_credential(&conn, id, label, fields_json).map_err(|error| {
        if error.to_string().starts_with("Graph資格情報") {
            ServiceError::validation(error.to_string())
        } else {
            ServiceError::external("トークンの更新に失敗しました", error)
        }
    })
}

pub fn delete_credential(state: &AppState, id: i64) -> ServiceResult<()> {
    let conn = lock_db(state)?;
    credentials::delete_credential(&conn, id).map_err(ServiceError::validation)
}

pub fn credential_usage(state: &AppState, id: i64) -> ServiceResult<Vec<CredentialUsage>> {
    let conn = lock_db(state)?;
    let out = credentials::find_usage(&conn, id)
        .map_err(|e| ServiceError::external("使用状況の取得に失敗しました", e))?
        .into_iter()
        .map(|(source_id, source_name)| CredentialUsage {
            source_id,
            source_name,
        })
        .collect();
    Ok(out)
}

pub fn credential_public_fields(state: &AppState, id: i64) -> ServiceResult<Value> {
    let conn = lock_db(state)?;
    let (label, fields) = credentials::public_fields(&conn, id)
        .map_err(|e| ServiceError::external("トークン情報の取得に失敗しました", e))?;
    Ok(serde_json::json!({ "label": label, "fields": fields }))
}

pub fn credential_permissions(
    state: &AppState,
    id: i64,
) -> ServiceResult<CredentialPermissionInspection> {
    let conn = lock_db(state)?;
    credentials::inspect_credential_permissions(&conn, id).map_err(|error| {
        ServiceError::validation(credential_permission_failure_message(&error.to_string()))
    })
}

fn credential_permission_failure_message(cause: &str) -> String {
    let reason = if cause.contains("接続に失敗")
        || cause.contains("timed out")
        || cause.contains("Transport")
    {
        "Microsoft への通信に失敗しました"
    } else if cause.contains("トークン取得失敗")
        || cause.contains("再認可")
        || cause.contains("認可が未設定")
    {
        "Microsoft がトークン発行を拒否しました。資格情報を確認し、委任認証の場合は再認可してください"
    } else if cause.contains("JWT") || cause.contains("scp") || cause.contains("roles") {
        "取得したトークンから権限情報を読み取れませんでした"
    } else if cause.contains("未設定") || cause.contains("見つかりません") {
        "保存済み資格情報の設定が不足しているか、削除されています"
    } else {
        "Microsoft の応答を確認できませんでした"
    };
    format!("承認済み権限の確認に失敗しました（{reason}）")
}

pub fn credential_secret_value(state: &AppState, id: i64) -> ServiceResult<String> {
    let conn = lock_db(state)?;
    credentials::resolve_secret_value(&conn, id)
        .map_err(|e| ServiceError::external("秘密値の取得に失敗しました", e))
}

pub fn request_box_oauth_cancel(state: &AppState) -> ServiceResult<()> {
    use std::sync::atomic::Ordering;
    let slot = state
        .box_oauth_cancel
        .lock()
        .map_err(|_| ServiceError::LockPoisoned)?;
    if let Some(flag) = slot.as_ref() {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

/// アプリ/ウィンドウ終了時: 実行中 OAuth を取消し、スロットを解放する。
pub fn cancel_box_oauth_on_shutdown(state: &AppState) {
    let _ = request_box_oauth_cancel(state);
    if let Ok(mut slot) = state.box_oauth_cancel.lock() {
        *slot = None;
    }
}

pub fn request_graph_oauth_cancel(state: &AppState) -> ServiceResult<()> {
    use std::sync::atomic::Ordering;
    let slot = state
        .graph_oauth_cancel
        .lock()
        .map_err(|_| ServiceError::LockPoisoned)?;
    if let Some(flag) = slot.as_ref() {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

pub fn cancel_graph_oauth_on_shutdown(state: &AppState) {
    let _ = request_graph_oauth_cancel(state);
    if let Ok(mut slot) = state.graph_oauth_cancel.lock() {
        *slot = None;
    }
}

#[cfg(test)]
mod tests {
    use super::{add_credential, credential_permission_failure_message, list_credentials};
    use crate::app_state::AppState;
    use crate::db;

    #[test]
    fn lists_credentials_by_scope() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        let state = AppState::new(conn);
        add_credential(&state, "notion", "個人", r#"{"token":"ntn_x"}"#).unwrap();
        let list = list_credentials(&state, "notion").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].label, "個人");
    }

    #[test]
    fn permission_failures_explain_safe_actionable_categories() {
        assert!(credential_permission_failure_message(
            "Microsoft 認証エンドポイントへの接続に失敗: offline"
        )
        .contains("通信に失敗"));
        assert!(credential_permission_failure_message(
            "Microsoft トークン取得失敗 400: invalid_grant"
        )
        .contains("再認可"));
        assert!(credential_permission_failure_message(
            "アクセストークンに scp クレームがありません"
        )
        .contains("権限情報を読み取れません"));
        assert!(!credential_permission_failure_message("secret-value").contains("secret-value"));
    }
}

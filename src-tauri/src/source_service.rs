//! データソース操作の業務処理。Tauri command 型へ依存しない。

use crate::app_state::AppState;
use crate::model::SourceInfo;
use crate::refresh_service::{reload_many_for_state, reload_one_source, ReloadSummary};
use crate::service_error::{lock_db, ServiceError, ServiceResult};
use crate::{credentials, db, secret};

fn validate_credential_compatibility(
    conn: &rusqlite::Connection,
    type_: &str,
    config_json: &str,
) -> ServiceResult<()> {
    credentials::validate_source_credential_compatibility(conn, type_, config_json)
        .map_err(|error| ServiceError::validation(error.to_string()))
}

pub fn list_sources(state: &AppState) -> ServiceResult<Vec<SourceInfo>> {
    let conn = lock_db(state)?;
    db::list_sources(&conn)
        .map_err(|e| ServiceError::external("データソース一覧の取得に失敗しました", e))
}

pub fn add_source(
    state: &AppState,
    type_: String,
    name: String,
    config_json: String,
) -> ServiceResult<i64> {
    let id = {
        let conn = lock_db(state)?;
        validate_credential_compatibility(&conn, &type_, &config_json)?;
        let config_json = secret::protect_config(&type_, &config_json)
            .map_err(|e| ServiceError::external("接続情報の保存に失敗しました", e))?;
        db::insert_source(&conn, &type_, &name, &config_json)
            .map_err(|e| ServiceError::external("データソースの追加に失敗しました", e))?
    };
    let summary = reload_one_source(state, id);
    summary.log_failures();
    Ok(id)
}

fn fill_shared_from_sibling(
    config_json: &str,
    sibling_config_json: &str,
    shared_keys: &[String],
) -> ServiceResult<String> {
    let sibling: serde_json::Value = serde_json::from_str(sibling_config_json)
        .map_err(|e| ServiceError::external("兄弟ソースの設定 JSON が不正です", e))?;
    let mut cfg: serde_json::Map<String, serde_json::Value> = serde_json::from_str(config_json)
        .map_err(|e| ServiceError::external("設定 JSON が不正です", e))?;
    for k in shared_keys {
        let cur = cfg.get(k).and_then(|v| v.as_str()).unwrap_or_default();
        if cur.is_empty() || cur == secret::SECRET_PLACEHOLDER {
            if let Some(v) = sibling.get(k) {
                cfg.insert(k.clone(), v.clone());
            }
        }
    }
    serde_json::to_string(&cfg)
        .map_err(|e| ServiceError::internal("設定 JSON の合成に失敗しました", e))
}

pub fn add_source_from_sibling(
    state: &AppState,
    sibling_id: i64,
    type_: String,
    name: String,
    config_json: String,
    shared_keys: Vec<String>,
) -> ServiceResult<i64> {
    let id = {
        let conn = lock_db(state)?;
        let (_, sibling_json) = db::get_source_config(&conn, sibling_id)
            .map_err(|e| ServiceError::external("兄弟ソースの取得に失敗しました", e))?;
        let merged = fill_shared_from_sibling(&config_json, &sibling_json, &shared_keys)?;
        validate_credential_compatibility(&conn, &type_, &merged)?;
        let protected = secret::protect_config(&type_, &merged)
            .map_err(|e| ServiceError::external("接続情報の保存に失敗しました", e))?;
        db::insert_source(&conn, &type_, &name, &protected)
            .map_err(|e| ServiceError::external("データソースの追加に失敗しました", e))?
    };
    let summary = reload_one_source(state, id);
    summary.log_failures();
    Ok(id)
}

pub fn delete_source(state: &AppState, id: i64) -> ServiceResult<()> {
    let conn = lock_db(state)?;
    db::delete_source(&conn, id)
        .map_err(|e| ServiceError::external("データソースの削除に失敗しました", e))
}

pub fn get_source_detail(state: &AppState, id: i64) -> ServiceResult<serde_json::Value> {
    let conn = lock_db(state)?;
    let (type_, name, refresh_interval, config_json) = db::get_source_full(&conn, id)
        .map_err(|e| ServiceError::external("データソース詳細の取得に失敗しました", e))?;
    let unlocked = state.is_unlocked();
    let visible = if unlocked {
        secret::unprotect_config(&config_json)
            .map_err(|e| ServiceError::external("接続情報の復号に失敗しました", e))?
    } else {
        secret::mask_config(&config_json)
            .map_err(|e| ServiceError::external("接続情報のマスクに失敗しました", e))?
    };
    let visible = crate::credentials::remask_cred_refs(&conn, &config_json, &visible)
        .map_err(|e| ServiceError::external("リンク済みトークン情報の取得に失敗しました", e))?;
    let linked_credentials = crate::credentials::cred_ref_labels(&conn, &config_json)
        .map_err(|e| ServiceError::external("リンク済みトークン一覧の取得に失敗しました", e))?;
    let config: serde_json::Value = serde_json::from_str(&visible)
        .map_err(|e| ServiceError::external("設定 JSON の解析に失敗しました", e))?;
    Ok(serde_json::json!({
        "type_": type_, "name": name, "refresh_interval": refresh_interval,
        "config": config, "unlocked": unlocked, "has_secret": secret::has_secret_fields(&type_),
        "linked_credentials": linked_credentials,
    }))
}

pub fn update_source(
    state: &AppState,
    id: i64,
    name: String,
    config_json: String,
) -> ServiceResult<()> {
    {
        let conn = lock_db(state)?;
        let (type_, old_config_json) = db::get_source_config(&conn, id)
            .map_err(|e| ServiceError::external("データソース設定の取得に失敗しました", e))?;
        let merged = secret::merge_preserving_placeholder(&type_, &old_config_json, &config_json)
            .map_err(|e| ServiceError::external("接続情報の更新に失敗しました", e))?;
        validate_credential_compatibility(&conn, &type_, &merged)?;
        let protected = secret::protect_config(&type_, &merged)
            .map_err(|e| ServiceError::external("接続情報の保存に失敗しました", e))?;
        db::update_source(&conn, id, &name, &protected)
            .map_err(|e| ServiceError::external("データソースの更新に失敗しました", e))?;
    }
    let summary = reload_one_source(state, id);
    summary.log_failures();
    Ok(())
}

pub fn duplicate_source(state: &AppState, id: i64, new_name: String) -> ServiceResult<i64> {
    let conn = lock_db(state)?;
    db::duplicate_source(&conn, id, &new_name)
        .map_err(|e| ServiceError::external("データソースの複製に失敗しました", e))
}

pub fn reorder_sources(state: &AppState, ordered_ids: Vec<i64>) -> ServiceResult<()> {
    let conn = lock_db(state)?;
    db::reorder_sources(&conn, &ordered_ids)
        .map_err(|e| ServiceError::external("並び順の保存に失敗しました", e))
}

pub fn preview_source_fields(
    state: &AppState,
    type_: String,
    config_json: String,
) -> ServiceResult<Vec<String>> {
    let resolved = {
        let conn = lock_db(state)?;
        validate_credential_compatibility(&conn, &type_, &config_json)?;
        crate::credentials::resolve_credentials_for_source_type(&conn, &type_, &config_json)
            .map_err(|e| ServiceError::external("接続情報の解決に失敗しました", e))?
    };
    crate::connectors::raw_field_names(&type_, &resolved)
        .map_err(|e| ServiceError::external("フィールド一覧の取得に失敗しました", e))
}

#[derive(serde::Serialize)]
pub struct FieldPreview {
    pub names: Vec<String>,
    pub from_cache: bool,
}

pub fn preview_source_fields_by_id(state: &AppState, id: i64) -> ServiceResult<FieldPreview> {
    let (type_, config_json) = {
        let conn = lock_db(state)?;
        let (type_, config_json) = db::get_source_config(&conn, id)
            .map_err(|e| ServiceError::external("データソース設定の取得に失敗しました", e))?;
        validate_credential_compatibility(&conn, &type_, &config_json)?;
        let resolved =
            crate::credentials::resolve_credentials_for_source_type(&conn, &type_, &config_json)
                .map_err(|e| ServiceError::external("接続情報の解決に失敗しました", e))?;
        (type_, resolved)
    };
    match crate::connectors::raw_field_names(&type_, &config_json) {
        Ok(names) => Ok(FieldPreview {
            names,
            from_cache: false,
        }),
        Err(live_err) => {
            let conn = lock_db(state)?;
            let cached = db::cached_field_names(&conn, id)
                .map_err(|e| ServiceError::external("キャッシュ列名の取得に失敗しました", e))?;
            if cached.is_empty() {
                Err(ServiceError::external(
                    "フィールド一覧の取得に失敗しました",
                    live_err,
                ))
            } else {
                Ok(FieldPreview {
                    names: cached,
                    from_cache: true,
                })
            }
        }
    }
}

pub fn set_refresh_interval(state: &AppState, id: i64, seconds: i64) -> ServiceResult<()> {
    let conn = lock_db(state)?;
    db::set_refresh_interval(&conn, id, seconds)
        .map_err(|e| ServiceError::external("更新間隔の保存に失敗しました", e))
}

pub fn reload_sources(state: &AppState, source_ids: Vec<i64>) -> ServiceResult<ReloadSummary> {
    let ids = if source_ids.is_empty() {
        let conn = lock_db(state)?;
        db::list_sources(&conn)
            .map_err(|e| ServiceError::external("データソース一覧の取得に失敗しました", e))?
            .into_iter()
            .map(|source| source.id)
            .collect()
    } else {
        source_ids
    };
    let summary = reload_many_for_state(state, &ids);
    summary.log_failures();
    Ok(summary)
}

#[cfg(test)]
mod source_service_tests {
    use super::{add_source, fill_shared_from_sibling};
    use crate::app_state::AppState;
    use crate::secret::SECRET_PLACEHOLDER;

    #[test]
    fn fill_shared_copies_encrypted_secret_without_decrypting() {
        let sibling = r#"{"token":"dpapi:deadbeef","database_id":"db1"}"#;
        let new_cfg = format!(r#"{{"database_id":"db2","token":"{SECRET_PLACEHOLDER}"}}"#);
        let out = fill_shared_from_sibling(&new_cfg, sibling, &["token".to_string()]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["token"], "dpapi:deadbeef");
        assert_eq!(v["database_id"], "db2");
    }

    #[test]
    fn fill_shared_keeps_explicitly_entered_value() {
        let sibling = r#"{"token":"dpapi:deadbeef"}"#;
        let new_cfg = r#"{"token":"ntn_typed_by_user","database_id":"db2"}"#;
        let out = fill_shared_from_sibling(new_cfg, sibling, &["token".to_string()]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["token"], "ntn_typed_by_user");
    }

    #[test]
    fn add_source_rejects_incompatible_graph_credential_before_insert() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::init_schema(&conn).unwrap();
        let credential_id = crate::credentials::add_credential(
            &conn,
            "graph",
            "SharePoint委任",
            r#"{"auth_method":"delegated","tenant_id":"t","client_id":"c","capabilities":["sharepoint"],"refresh_token":"rt"}"#,
        )
        .unwrap();
        let state = AppState::new(conn);
        let error = add_source(
            &state,
            "graph_teams".to_string(),
            "Teams".to_string(),
            serde_json::json!({ "client_secret": format!("cred:{credential_id}") }).to_string(),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("委任 OAuth に対応していません"));
        assert!(crate::db::list_sources(&state.db.lock().unwrap())
            .unwrap()
            .is_empty());
    }
}

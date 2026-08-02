//! ブラウズ（外部一覧取得）の業務処理。Tauri 型へ依存しない。

use rusqlite::Connection;
use serde_json::Map;

use crate::app_state::AppState;
use crate::browse::{
    self, AirtableBaseEntry, AirtableTableEntry, BrowserProfileEntry, ExcelGridPreview,
    GraphAuthArgs, NotionDbEntry, SiteEntry, SiteResourceEntry, TeamEntry,
};
use crate::connectors;
use crate::credentials;
use crate::db;
use crate::secret;
use crate::service_error::{lock_db, ServiceError, ServiceResult};

fn resolve_and_decrypt(conn: &Connection, config_json: &str) -> ServiceResult<String> {
    let resolved = credentials::resolve_credentials(conn, config_json)
        .map_err(|e| ServiceError::external("接続情報の解決に失敗しました", e))?;
    secret::unprotect_config(&resolved)
        .map_err(|e| ServiceError::external("接続情報の復号に失敗しました", e))
}

/// 復号済み設定に、ダイアログ上の編集値を重ねる。空・プレースホルダーは未編集として無視する。
pub fn merge_overrides(
    config_json: &str,
    overrides_json: &str,
) -> ServiceResult<Map<String, serde_json::Value>> {
    let mut cfg: Map<String, serde_json::Value> = serde_json::from_str(config_json)
        .map_err(|e| ServiceError::external("設定 JSON が不正です", e))?;
    let overrides: Map<String, serde_json::Value> = serde_json::from_str(overrides_json)
        .map_err(|e| ServiceError::external("上書き JSON が不正です", e))?;
    for (k, v) in overrides {
        let s = v.as_str().unwrap_or_default();
        if s.is_empty() || s == secret::SECRET_PLACEHOLDER {
            continue;
        }
        cfg.insert(k, v);
    }
    Ok(cfg)
}

pub fn effective_config(
    conn: &Connection,
    id: i64,
    overrides_json: &str,
) -> ServiceResult<Map<String, serde_json::Value>> {
    let (_, config_json) = db::get_source_config(conn, id)
        .map_err(|e| ServiceError::external("ソース設定の取得に失敗しました", e))?;
    let decrypted = resolve_and_decrypt(conn, &config_json)?;
    let merged = merge_overrides(&decrypted, overrides_json)?;
    let merged_json = serde_json::to_string(&merged)
        .map_err(|e| ServiceError::internal("設定の合成に失敗しました", e))?;
    let merged_decrypted = resolve_and_decrypt(conn, &merged_json)?;
    serde_json::from_str(&merged_decrypted)
        .map_err(|e| ServiceError::external("設定 JSON の解析に失敗しました", e))
}

fn cfg_str(cfg: &Map<String, serde_json::Value>, key: &str) -> String {
    cfg.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

pub fn browse_notion_databases(token: &str) -> ServiceResult<Vec<NotionDbEntry>> {
    browse::notion_list_databases(token)
        .map_err(|e| ServiceError::external("Notion データベース一覧の取得に失敗しました", e))
}

pub fn browse_notion_databases_for_source(
    state: &AppState,
    id: i64,
    overrides_json: &str,
) -> ServiceResult<Vec<NotionDbEntry>> {
    let conn = lock_db(state)?;
    let cfg = effective_config(&conn, id, overrides_json)?;
    drop(conn);
    let token = cfg_str(&cfg, "token");
    if token.is_empty() {
        return Err(ServiceError::validation(
            "インテグレーショントークンが設定されていません",
        ));
    }
    browse_notion_databases(&token)
}

pub fn browse_airtable_bases(token: &str) -> ServiceResult<Vec<AirtableBaseEntry>> {
    browse::airtable_list_bases(token)
        .map_err(|e| ServiceError::external("Airtable Base 一覧の取得に失敗しました", e))
}

pub fn browse_airtable_bases_for_source(
    state: &AppState,
    id: i64,
    overrides_json: &str,
) -> ServiceResult<Vec<AirtableBaseEntry>> {
    let conn = lock_db(state)?;
    let cfg = effective_config(&conn, id, overrides_json)?;
    drop(conn);
    let token = cfg_str(&cfg, "token");
    if token.is_empty() {
        return Err(ServiceError::validation(
            "Personal Access Token が設定されていません",
        ));
    }
    browse_airtable_bases(&token)
}

pub fn browse_airtable_tables(
    token: &str,
    base_id: &str,
) -> ServiceResult<Vec<AirtableTableEntry>> {
    browse::airtable_list_tables(token, base_id)
        .map_err(|e| ServiceError::external("Airtable テーブル一覧の取得に失敗しました", e))
}

pub fn browse_airtable_tables_for_source(
    state: &AppState,
    id: i64,
    overrides_json: &str,
    base_id: Option<String>,
) -> ServiceResult<Vec<AirtableTableEntry>> {
    let conn = lock_db(state)?;
    let cfg = effective_config(&conn, id, overrides_json)?;
    drop(conn);
    let token = cfg_str(&cfg, "token");
    if token.is_empty() {
        return Err(ServiceError::validation(
            "Personal Access Token が設定されていません",
        ));
    }
    let target_base = match base_id {
        Some(b) if !b.is_empty() => b,
        _ => cfg_str(&cfg, "base_id"),
    };
    if target_base.is_empty() {
        return Err(ServiceError::validation("Base ID が指定されていません"));
    }
    browse_airtable_tables(&token, &target_base)
}

pub fn browse_graph_site_resources_for_source(
    state: &AppState,
    id: i64,
    overrides_json: &str,
) -> ServiceResult<Vec<SiteResourceEntry>> {
    let conn = lock_db(state)?;
    let cfg = effective_config(&conn, id, overrides_json)?;
    drop(conn);
    let auth = GraphAuthArgs {
        tenant_id: cfg_str(&cfg, "tenant_id"),
        client_id: cfg_str(&cfg, "client_id"),
        client_secret: cfg_str(&cfg, "client_secret"),
    };
    let site_url = cfg_str(&cfg, "site_url");
    if site_url.is_empty() {
        return Err(ServiceError::validation("サイト URL が設定されていません"));
    }
    browse_graph_site_resources(&auth, &site_url)
}

pub fn browse_graph_sites(auth: &GraphAuthArgs) -> ServiceResult<Vec<SiteEntry>> {
    browse::graph_list_sites(auth)
        .map_err(|e| ServiceError::external("SharePoint サイト一覧の取得に失敗しました", e))
}

pub fn browse_graph_teams(auth: &GraphAuthArgs) -> ServiceResult<Vec<TeamEntry>> {
    browse::graph_list_teams(auth)
        .map_err(|e| ServiceError::external("Teams 一覧の取得に失敗しました", e))
}

pub fn browse_graph_team_site_url(
    auth: &GraphAuthArgs,
    team_group_id: &str,
) -> ServiceResult<String> {
    browse::graph_team_site_url(auth, team_group_id)
        .map_err(|e| ServiceError::external("Teams サイト URL の取得に失敗しました", e))
}

pub fn browse_graph_site_resources(
    auth: &GraphAuthArgs,
    site_url: &str,
) -> ServiceResult<Vec<SiteResourceEntry>> {
    browse::graph_list_site_resources(auth, site_url).map_err(|e| {
        ServiceError::external("SharePoint リスト/ライブラリ一覧の取得に失敗しました", e)
    })
}

pub fn browse_excel_sheets(path: &str) -> ServiceResult<Vec<String>> {
    browse::list_excel_sheets(&connectors::strip_path_quotes(path))
        .map_err(|e| ServiceError::external("Excel シート一覧の取得に失敗しました", e))
}

pub fn browse_excel_tables(path: &str) -> ServiceResult<Vec<String>> {
    browse::list_excel_tables(&connectors::strip_path_quotes(path))
        .map_err(|e| ServiceError::external("Excel テーブル一覧の取得に失敗しました", e))
}

pub fn browse_excel_grid(
    path: &str,
    sheet: &str,
    rows: usize,
    cols: usize,
    mode: &str,
) -> ServiceResult<ExcelGridPreview> {
    browse::preview_excel_grid(
        &connectors::strip_path_quotes(path),
        sheet,
        rows,
        cols,
        mode,
    )
    .map_err(|e| ServiceError::external("Excel セルプレビューの取得に失敗しました", e))
}

pub fn browse_sqlite_tables(path: &str) -> ServiceResult<Vec<String>> {
    browse::list_sqlite_tables(&connectors::strip_path_quotes(path))
        .map_err(|e| ServiceError::external("SQLite テーブル一覧の取得に失敗しました", e))
}

pub fn browse_accdb_tables(path: &str) -> ServiceResult<Vec<String>> {
    browse::list_accdb_tables(&connectors::strip_path_quotes(path))
        .map_err(|e| ServiceError::external("Access テーブル一覧の取得に失敗しました", e))
}

pub fn browse_browser_profiles(
    browser: &str,
    target: &str,
) -> ServiceResult<Vec<BrowserProfileEntry>> {
    browse::browse_browser_profiles(browser, target)
        .map_err(|e| ServiceError::external("ブラウザプロファイル一覧の取得に失敗しました", e))
}

#[cfg(test)]
mod parent_dialog_tests {
    use super::{effective_config, merge_overrides};
    use crate::secret::SECRET_PLACEHOLDER;

    #[test]
    fn merge_overrides_ignores_masked_and_empty_values() {
        let saved = r#"{"token":"ntn_real","database_id":"db1"}"#;
        let edited = format!(r#"{{"token":"{SECRET_PLACEHOLDER}","database_id":""}}"#);
        let merged = merge_overrides(saved, &edited).unwrap();
        assert_eq!(merged["token"], "ntn_real");
        assert_eq!(merged["database_id"], "db1");
    }

    #[test]
    fn merge_overrides_applies_fresh_cred_reference_as_a_real_edit() {
        let saved_and_resolved = r#"{"token":"ntn_old_resolved_value","database_id":"db1"}"#;
        let overrides = r#"{"token":"cred:7","database_id":"db2"}"#;
        let merged = merge_overrides(saved_and_resolved, overrides).unwrap();
        assert_eq!(merged["token"], "cred:7");
        assert_eq!(merged["database_id"], "db2");
    }

    #[test]
    fn merge_overrides_applies_edited_values() {
        let saved = r#"{"token":"ntn_old","site_url":"https://old"}"#;
        let edited = r#"{"token":"ntn_new","site_url":"https://new","extra":"x"}"#;
        let merged = merge_overrides(saved, edited).unwrap();
        assert_eq!(merged["token"], "ntn_new");
        assert_eq!(merged["site_url"], "https://new");
        assert_eq!(merged["extra"], "x");
    }

    #[test]
    fn effective_config_resolves_a_freshly_overridden_credential_reference() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::init_schema(&conn).unwrap();
        let old_id =
            crate::credentials::add_credential(&conn, "notion", "旧", r#"{"token":"ntn_old"}"#)
                .unwrap();
        let new_id =
            crate::credentials::add_credential(&conn, "notion", "新", r#"{"token":"ntn_new"}"#)
                .unwrap();
        let source_id = crate::db::insert_source(
            &conn,
            "notion",
            "テスト",
            &format!(r#"{{"token":"cred:{old_id}","database_id":"db1"}}"#),
        )
        .unwrap();

        let overrides = format!(r#"{{"token":"cred:{new_id}"}}"#);
        let cfg = effective_config(&conn, source_id, &overrides).unwrap();
        assert_eq!(cfg["token"], "ntn_new");
        assert_eq!(cfg["database_id"], "db1");

        let cfg2 = effective_config(&conn, source_id, "{}").unwrap();
        assert_eq!(cfg2["token"], "ntn_old");
    }

    #[test]
    #[ignore = "requires EVERYDB_NOTION_TOKEN"]
    fn notion_live_list_via_saved_config_even_when_masked() {
        use crate::{browse, db, secret};
        let token = std::env::var("EVERYDB_NOTION_TOKEN").expect("EVERYDB_NOTION_TOKEN");
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        let cfg = serde_json::json!({ "token": token, "database_id": "" }).to_string();
        let protected = secret::protect_config("notion", &cfg).unwrap();
        assert!(protected.contains("dpapi:"));
        let id = db::insert_source(&conn, "notion", "案件DB", &protected).unwrap();

        let overrides = format!(r#"{{"token":"{SECRET_PLACEHOLDER}"}}"#);
        let effective = effective_config(&conn, id, &overrides).unwrap();
        let effective_token = effective["token"].as_str().unwrap();
        assert_ne!(effective_token, SECRET_PLACEHOLDER);

        let dbs = browse::notion_list_databases(effective_token).unwrap();
        assert!(!dbs.is_empty());
    }
}

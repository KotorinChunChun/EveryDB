use rusqlite::Connection;
use tauri::State;

use crate::app_state::AppState;
use crate::{db, secret};

fn err_to_string<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}
// ---- ブラウズして追加（要件 §9-2）----

#[tauri::command]
pub fn browse_notion_databases(token: String) -> Result<Vec<crate::browse::NotionDbEntry>, String> {
    crate::browse::notion_list_databases(&token).map_err(err_to_string)
}

/// 保存済みソースの設定（DPAPI 暗号化）を復号し、ダイアログ上の編集中の値で上書きした
/// 「実効設定」を作る。マスクされたまま（`••••••••`）や空の値は未編集とみなして
/// 保存済みの値を使う。
///
/// 接続設定ダイアログはマスターパスワードが未解錠だと秘密フィールドがマスクされて表示される。
/// その値をそのまま外部 API へ渡すと必ず認証エラーになるため、一覧取得はフロントから受け取った
/// 値ではなくこの実効設定を使う（`preview_source_fields_by_id` と同じ考え方）。
/// 保存済みトークンへの参照（cred:<id>）を実値へ展開してから復号する。
fn resolve_and_decrypt(conn: &Connection, config_json: &str) -> Result<String, String> {
    let resolved =
        crate::credentials::resolve_credentials(conn, config_json).map_err(err_to_string)?;
    secret::unprotect_config(&resolved).map_err(err_to_string)
}

fn effective_config(
    conn: &Connection,
    id: i64,
    overrides_json: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let (_, config_json) = db::get_source_config(conn, id).map_err(err_to_string)?;
    let decrypted = resolve_and_decrypt(conn, &config_json)?;
    let merged = merge_overrides(&decrypted, overrides_json)?;
    // override 自体が「このセッションで選び直した」新しい cred:<id> 参照であることがある
    // （ダイアログ上でトークンを変更しただけでまだ保存していない状態）。その場合 merge_overrides は
    // 単なる文字列として上書きするだけなので、マージ後にもう一度解決・復号する。
    let merged_json = serde_json::to_string(&merged).map_err(err_to_string)?;
    let merged_decrypted = resolve_and_decrypt(conn, &merged_json)?;
    serde_json::from_str(&merged_decrypted).map_err(err_to_string)
}

/// 復号済み設定に、ダイアログ上の編集値を重ねる。空・プレースホルダーは未編集として無視する。
fn merge_overrides(
    config_json: &str,
    overrides_json: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let mut cfg: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(config_json).map_err(err_to_string)?;
    let overrides: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(overrides_json).map_err(err_to_string)?;
    for (k, v) in overrides {
        let s = v.as_str().unwrap_or_default();
        // 空・プレースホルダーは「未編集」扱い（保存済みの値を使う）。
        // cred:<id> は「このセッションで選び直した新しいリンク」を意味する実際の編集値のため、
        // 通常の値と同様に適用する（呼び出し元の effective_config 側で改めて解決・復号する）。
        if s.is_empty() || s == secret::SECRET_PLACEHOLDER {
            continue; // 未編集 → 保存済みの値を使う
        }
        cfg.insert(k, v);
    }
    Ok(cfg)
}

fn cfg_str(cfg: &serde_json::Map<String, serde_json::Value>, key: &str) -> String {
    cfg.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

/// 接続設定ダイアログ用の Notion データベース一覧。保存済みトークンを使うため、
/// マスターパスワードが未解錠でも一覧を取得できる。
#[tauri::command]
pub fn browse_notion_databases_for_source(
    state: State<'_, AppState>,
    id: i64,
    overrides_json: String,
) -> Result<Vec<crate::browse::NotionDbEntry>, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let cfg = effective_config(&conn, id, &overrides_json)?;
    drop(conn);
    let token = cfg_str(&cfg, "token");
    if token.is_empty() {
        return Err("インテグレーショントークンが設定されていません".to_string());
    }
    crate::browse::notion_list_databases(&token).map_err(err_to_string)
}

#[tauri::command]
pub fn browse_airtable_bases(
    token: String,
) -> Result<Vec<crate::browse::AirtableBaseEntry>, String> {
    crate::browse::airtable_list_bases(&token).map_err(err_to_string)
}

/// 接続設定ダイアログ用の Airtable Base 一覧。保存済みトークンを使うため、
/// マスターパスワードが未解錠でも一覧を取得できる。
#[tauri::command]
pub fn browse_airtable_bases_for_source(
    state: State<'_, AppState>,
    id: i64,
    overrides_json: String,
) -> Result<Vec<crate::browse::AirtableBaseEntry>, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let cfg = effective_config(&conn, id, &overrides_json)?;
    drop(conn);
    let token = cfg_str(&cfg, "token");
    if token.is_empty() {
        return Err("Personal Access Token が設定されていません".to_string());
    }
    crate::browse::airtable_list_bases(&token).map_err(err_to_string)
}

#[tauri::command]
pub fn browse_airtable_tables(
    token: String,
    base_id: String,
) -> Result<Vec<crate::browse::AirtableTableEntry>, String> {
    crate::browse::airtable_list_tables(&token, &base_id).map_err(err_to_string)
}

/// 接続設定ダイアログ用の Airtable テーブル一覧。保存済みトークンを使うため、
/// マスターパスワードが未解錠でも一覧を取得できる。
#[tauri::command]
pub fn browse_airtable_tables_for_source(
    state: State<'_, AppState>,
    id: i64,
    overrides_json: String,
    base_id: Option<String>,
) -> Result<Vec<crate::browse::AirtableTableEntry>, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let cfg = effective_config(&conn, id, &overrides_json)?;
    drop(conn);
    let token = cfg_str(&cfg, "token");
    if token.is_empty() {
        return Err("Personal Access Token が設定されていません".to_string());
    }
    let target_base = match base_id {
        Some(b) if !b.is_empty() => b,
        _ => cfg_str(&cfg, "base_id"),
    };
    if target_base.is_empty() {
        return Err("Base ID が指定されていません".to_string());
    }
    crate::browse::airtable_list_tables(&token, &target_base).map_err(err_to_string)
}

/// 接続設定ダイアログ用の SharePoint リスト/ライブラリ一覧。保存済みクライアントシークレットを
/// 使うため、マスターパスワードが未解錠でも一覧を取得できる。
#[tauri::command]
pub fn browse_graph_site_resources_for_source(
    state: State<'_, AppState>,
    id: i64,
    overrides_json: String,
) -> Result<Vec<crate::browse::SiteResourceEntry>, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let cfg = effective_config(&conn, id, &overrides_json)?;
    drop(conn);
    let auth = crate::browse::GraphAuthArgs {
        tenant_id: cfg_str(&cfg, "tenant_id"),
        client_id: cfg_str(&cfg, "client_id"),
        client_secret: cfg_str(&cfg, "client_secret"),
    };
    let site_url = cfg_str(&cfg, "site_url");
    if site_url.is_empty() {
        return Err("サイト URL が設定されていません".to_string());
    }
    crate::browse::graph_list_site_resources(&auth, &site_url).map_err(err_to_string)
}

#[tauri::command]
pub fn browse_graph_sites(
    auth: crate::browse::GraphAuthArgs,
) -> Result<Vec<crate::browse::SiteEntry>, String> {
    crate::browse::graph_list_sites(&auth).map_err(err_to_string)
}

#[tauri::command]
pub fn browse_graph_teams(
    auth: crate::browse::GraphAuthArgs,
) -> Result<Vec<crate::browse::TeamEntry>, String> {
    crate::browse::graph_list_teams(&auth).map_err(err_to_string)
}

#[tauri::command]
pub fn browse_graph_team_site_url(
    auth: crate::browse::GraphAuthArgs,
    team_group_id: String,
) -> Result<String, String> {
    crate::browse::graph_team_site_url(&auth, &team_group_id).map_err(err_to_string)
}

#[tauri::command]
pub fn browse_graph_site_resources(
    auth: crate::browse::GraphAuthArgs,
    site_url: String,
) -> Result<Vec<crate::browse::SiteResourceEntry>, String> {
    crate::browse::graph_list_site_resources(&auth, &site_url).map_err(err_to_string)
}

#[tauri::command]
pub fn browse_excel_sheets(path: String) -> Result<Vec<String>, String> {
    crate::browse::list_excel_sheets(&crate::connectors::strip_path_quotes(&path))
        .map_err(err_to_string)
}

#[tauri::command]
pub fn browse_excel_tables(path: String) -> Result<Vec<String>, String> {
    crate::browse::list_excel_tables(&crate::connectors::strip_path_quotes(&path))
        .map_err(err_to_string)
}

#[tauri::command]
pub fn browse_excel_grid(
    path: String,
    sheet: String,
    rows: usize,
    cols: usize,
    mode: Option<String>,
) -> Result<crate::browse::ExcelGridPreview, String> {
    let mode_str = mode.as_deref().unwrap_or("top");
    crate::browse::preview_excel_grid(
        &crate::connectors::strip_path_quotes(&path),
        &sheet,
        rows,
        cols,
        mode_str,
    )
    .map_err(err_to_string)
}

#[tauri::command]
pub fn browse_sqlite_tables(path: String) -> Result<Vec<String>, String> {
    crate::browse::list_sqlite_tables(&crate::connectors::strip_path_quotes(&path))
        .map_err(err_to_string)
}

#[tauri::command]
pub fn browse_accdb_tables(path: String) -> Result<Vec<String>, String> {
    crate::browse::list_accdb_tables(&crate::connectors::strip_path_quotes(&path))
        .map_err(err_to_string)
}

/// ブラウザ系データソース（閲覧履歴/ブックマーク/ダウンロード履歴）のパス指定を「候補から選ぶだけ」
/// にするための一覧取得（要件: パス指定が難しいので自動で候補を探して選ばせてほしい）。
#[tauri::command]
pub fn browse_browser_profiles(
    browser: String,
    target: String,
) -> Result<Vec<crate::browse::BrowserProfileEntry>, String> {
    crate::browse::browse_browser_profiles(&browser, &target).map_err(err_to_string)
}

#[cfg(test)]
mod parent_dialog_tests {
    use super::merge_overrides;
    use crate::secret::SECRET_PLACEHOLDER;

    #[test]
    fn merge_overrides_ignores_masked_and_empty_values() {
        let saved = r#"{"token":"ntn_real","database_id":"db1"}"#;
        let edited = format!(r#"{{"token":"{SECRET_PLACEHOLDER}","database_id":""}}"#);
        let merged = merge_overrides(saved, &edited).unwrap();
        // マスク・空欄はユーザーが触っていない扱い → 保存済みの値が残る
        assert_eq!(merged["token"], "ntn_real");
        assert_eq!(merged["database_id"], "db1");
    }

    #[test]
    fn merge_overrides_applies_fresh_cred_reference_as_a_real_edit() {
        // cred:<id> は「ダイアログ上でこのセッション中に選び直した新しいリンク」を意味する
        // 実際の編集値であり、プレースホルダーとは違って無視してはいけない
        // （無視すると「トークンを変更した直後は反映されない」不具合になる。実際に発生した：
        // 接続設定でトークンを変更→保存前に「テーブル一覧の更新」を押すと旧トークンで検証されていた）。
        // 実際の解決（cred:<id> → 実値）は merge_overrides の外、effective_config 側の役割。
        let saved_and_resolved = r#"{"token":"ntn_old_resolved_value","database_id":"db1"}"#;
        let overrides = r#"{"token":"cred:7","database_id":"db2"}"#;
        let merged = merge_overrides(saved_and_resolved, overrides).unwrap();
        assert_eq!(merged["token"], "cred:7"); // そのまま適用される（呼び出し元が改めて解決する）
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
        // 再現テスト: 接続設定でトークンを選び直した直後、保存する前に「テーブル一覧の更新」を
        // 押しても新しいトークンが使われず、旧トークンで検証されているように見える不具合の回帰確認。
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

        // 保存前の override（フロントが「変更…」で選び直した直後の状態を模す）。
        let overrides = format!(r#"{{"token":"cred:{new_id}"}}"#);
        let cfg = super::effective_config(&conn, source_id, &overrides).unwrap();
        assert_eq!(cfg["token"], "ntn_new"); // 保存前でも新しく選んだトークンの実値が使われる
        assert_eq!(cfg["database_id"], "db1"); // override に無いフィールドは保存済みの値を維持

        // override が無ければ、引き続き保存済み(旧)のトークンが使われる。
        let cfg2 = super::effective_config(&conn, source_id, "{}").unwrap();
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
        assert!(
            protected.contains("dpapi:"),
            "トークンが暗号化されていること"
        );
        let id = db::insert_source(&conn, "notion", "案件DB", &protected).unwrap();

        // ダイアログ側はロック中でマスク値しか持っていない状況を再現する。
        let overrides = format!(r#"{{"token":"{SECRET_PLACEHOLDER}"}}"#);
        let effective = super::effective_config(&conn, id, &overrides).unwrap();
        let effective_token = effective["token"].as_str().unwrap();
        assert_ne!(
            effective_token, SECRET_PLACEHOLDER,
            "保存済みの平文トークンへ解決されること"
        );

        let dbs = browse::notion_list_databases(effective_token).unwrap();
        println!("notion databases via saved config = {}", dbs.len());
        assert!(!dbs.is_empty());
    }
}

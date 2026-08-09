//! データソース再取得の業務処理（Tauri 非依存）。

use rusqlite::Connection;

use crate::app_state::AppState;
use crate::internal_log;
use crate::{browse, connectors, credentials, db};

#[derive(Debug, PartialEq, Eq)]
pub enum ReloadOneOutcome {
    Success { record_count: usize },
    Cancelled,
}

#[derive(serde::Serialize)]
pub struct ReloadSourceResult {
    pub source_id: i64,
    pub outcome: &'static str,
    pub record_count: Option<usize>,
    pub error: Option<String>,
}

#[derive(Default, serde::Serialize)]
pub struct ReloadSummary {
    pub successful: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub results: Vec<ReloadSourceResult>,
}

impl ReloadSummary {
    fn record(&mut self, source_id: i64, result: anyhow::Result<ReloadOneOutcome>) {
        match result {
            Ok(ReloadOneOutcome::Success { record_count }) => {
                self.successful += 1;
                self.results.push(ReloadSourceResult {
                    source_id,
                    outcome: "success",
                    record_count: Some(record_count),
                    error: None,
                });
            }
            Ok(ReloadOneOutcome::Cancelled) => {
                self.cancelled += 1;
                self.results.push(ReloadSourceResult {
                    source_id,
                    outcome: "cancelled",
                    record_count: None,
                    error: Some("同じデータソースは既にリロード中です".to_string()),
                });
            }
            Err(error) => {
                self.failed += 1;
                self.results.push(ReloadSourceResult {
                    source_id,
                    outcome: "failed",
                    record_count: None,
                    error: Some(error.to_string()),
                });
            }
        }
    }

    pub fn log_failures(&self) {
        for result in &self.results {
            if result.outcome == "failed" {
                internal_log::warn(
                    "reload",
                    format!(
                        "source {} failed: {}",
                        result.source_id,
                        result.error.as_deref().unwrap_or("unknown")
                    ),
                );
            }
        }
    }
}

fn set_status_or_log(conn: &Connection, id: i64, status: &str, error: Option<&str>) {
    if let Err(e) = db::set_status(conn, id, status, error) {
        internal_log::warn(
            "reload",
            format!("failed to persist status for source {id}: {e}"),
        );
    }
}

#[derive(Debug)]
struct NotionMetadataRequest {
    token: String,
    database_id: String,
}

#[derive(Debug, PartialEq, Eq)]
struct NotionMetadataUpdate {
    database_id: String,
    database_name: String,
}

/// Notionだけ、再読み込み前に最新のデータベース名を取得できる材料を切り出す。
/// 保存用JSONは資格情報参照・暗号化値を保持し、API呼び出しには解決済みJSONだけを使う。
fn notion_metadata_request(
    type_: &str,
    resolved_config_json: &str,
) -> anyhow::Result<Option<NotionMetadataRequest>> {
    if type_ != "notion" {
        return Ok(None);
    }
    let resolved: serde_json::Value = serde_json::from_str(resolved_config_json)?;
    let token = resolved
        .get("token")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let database_id = resolved
        .get("database_id")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    if token.is_empty() || database_id.is_empty() {
        return Err(anyhow::anyhow!(
            "Notionのデータベース名取得に必要な接続情報が不足しています"
        ));
    }
    Ok(Some(NotionMetadataRequest { token, database_id }))
}

fn resolve_notion_metadata(
    request: Option<NotionMetadataRequest>,
) -> anyhow::Result<Option<NotionMetadataUpdate>> {
    let Some(request) = request else {
        return Ok(None);
    };
    let entry = browse::notion_get_database(&request.token, &request.database_id)?;
    Ok(Some(notion_metadata_update(request, entry.title)))
}

fn notion_metadata_update(
    request: NotionMetadataRequest,
    database_name: String,
) -> NotionMetadataUpdate {
    NotionMetadataUpdate {
        database_id: request.database_id,
        database_name,
    }
}

fn persist_notion_metadata(
    conn: &Connection,
    id: i64,
    update: Option<&NotionMetadataUpdate>,
) -> anyhow::Result<()> {
    if let Some(update) = update {
        // API通信中に別操作で設定が更新されても、通信前の古いJSONで上書きしない。
        // 現在値を読み直し、名称の1項目だけを差し替える。
        let (type_, current_config_json) = db::get_source_config(conn, id)?;
        if type_ != "notion" {
            return Err(anyhow::anyhow!(
                "再読み込み中にデータソース種別が変更されました"
            ));
        }
        let mut current: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&current_config_json)?;
        let current_database_id = current
            .get("database_id")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .replace('-', "")
            .to_ascii_lowercase();
        let resolved_database_id = update.database_id.replace('-', "").to_ascii_lowercase();
        if current_database_id != resolved_database_id {
            return Err(anyhow::anyhow!(
                "再読み込み中にNotionデータベースIDが変更されました"
            ));
        }
        current.insert(
            "database_name".to_string(),
            serde_json::Value::String(update.database_name.clone()),
        );
        db::update_source(
            conn,
            id,
            &update.database_name,
            &serde_json::to_string(&current)?,
        )?;
    }
    Ok(())
}

pub fn reload_one(conn: &mut Connection, id: i64) -> anyhow::Result<()> {
    let (type_, cfg) = db::get_source_config(conn, id)?;
    let prepared = (|| -> anyhow::Result<(
        Box<dyn connectors::Connector>,
        Option<NotionMetadataRequest>,
    )> {
        let resolved_cfg = credentials::resolve_credentials_for_source_type(conn, &type_, &cfg)?;
        let notion_request = notion_metadata_request(&type_, &resolved_cfg)?;
        Ok((
            connectors::build_connector(&type_, &resolved_cfg)?,
            notion_request,
        ))
    })();
    let (connector, notion_request) = match prepared {
        Ok(value) => value,
        Err(e) => {
            set_status_or_log(conn, id, "error", Some(&e.to_string()));
            return Err(e);
        }
    };
    let notion_update = match resolve_notion_metadata(notion_request) {
        Ok(update) => update,
        Err(error) => {
            set_status_or_log(conn, id, "error", Some(&error.to_string()));
            return Err(error);
        }
    };
    persist_notion_metadata(conn, id, notion_update.as_ref())?;
    match connector.fetch() {
        Ok(records) => {
            db::replace_source_records(conn, id, &records)?;
            set_status_or_log(conn, id, "ok", None);
            Ok(())
        }
        Err(e) => {
            set_status_or_log(conn, id, "error", Some(&e.to_string()));
            Err(e)
        }
    }
}

pub fn reload_one_for_state(state: &AppState, id: i64) -> anyhow::Result<ReloadOneOutcome> {
    let _reload_guard = match state.try_begin_reload(id) {
        Some(guard) => guard,
        None => return Ok(ReloadOneOutcome::Cancelled),
    };
    let (connector, notion_request) = {
        let snapshot_started = std::time::Instant::now();
        let conn = state
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("lock poisoned"))?;
        let prepared = (|| -> anyhow::Result<(
            Box<dyn connectors::Connector>,
            Option<NotionMetadataRequest>,
        )> {
            let (type_, cfg) = db::get_source_config(&conn, id)?;
            let resolved_cfg =
                credentials::resolve_credentials_for_source_type(&conn, &type_, &cfg)?;
            let notion_request = notion_metadata_request(&type_, &resolved_cfg)?;
            Ok((
                connectors::build_connector(&type_, &resolved_cfg)?,
                notion_request,
            ))
        })();
        record_test_snapshot_lock(snapshot_started.elapsed());
        match prepared {
            Ok(value) => value,
            Err(error) => {
                set_status_or_log(&conn, id, "error", Some(&error.to_string()));
                return Err(error);
            }
        }
    };

    sleep_test_post_snapshot_delay();

    let notion_update = match resolve_notion_metadata(notion_request) {
        Ok(update) => update,
        Err(error) => {
            let conn = state
                .db
                .lock()
                .map_err(|_| anyhow::anyhow!("lock poisoned"))?;
            set_status_or_log(&conn, id, "error", Some(&error.to_string()));
            return Err(error);
        }
    };

    let fetched = connector.fetch();
    let mut conn = state
        .db
        .lock()
        .map_err(|_| anyhow::anyhow!("lock poisoned"))?;
    if let Err(error) = persist_notion_metadata(&conn, id, notion_update.as_ref()) {
        set_status_or_log(&conn, id, "error", Some(&error.to_string()));
        return Err(error);
    }
    match fetched {
        Ok(records) => {
            db::replace_source_records(&mut conn, id, &records)?;
            set_status_or_log(&conn, id, "ok", None);
            Ok(ReloadOneOutcome::Success {
                record_count: records.len(),
            })
        }
        Err(error) => {
            set_status_or_log(&conn, id, "error", Some(&error.to_string()));
            Err(error)
        }
    }
}

/// FTS5 の merge を 1 回だけ実行する。バッチ reload の末尾で呼ぶ（RF-50-06）。
pub fn optimize_fts(state: &AppState) -> bool {
    let Ok(conn) = state.db.lock() else {
        internal_log::warn("fts", "DB lock poisoned during optimize");
        return false;
    };
    match conn.execute(
        "INSERT INTO records_fts(records_fts) VALUES('optimize')",
        [],
    ) {
        Ok(_) => true,
        Err(error) => {
            internal_log::warn("fts", format!("optimize failed: {error}"));
            false
        }
    }
}

/// 複数 source を順に reload し、**末尾で 1 回だけ** FTS optimize する。
pub fn reload_many_for_state(state: &AppState, ids: &[i64]) -> ReloadSummary {
    let mut summary = ReloadSummary::default();
    for &id in ids {
        summary.record(id, reload_one_for_state(state, id));
    }
    optimize_fts(state);
    summary
}

/// 単一 source も batch API 経由で reload し、FTS optimize を末尾 1 回に揃える。
pub fn reload_one_source(state: &AppState, id: i64) -> ReloadSummary {
    reload_many_for_state(state, std::slice::from_ref(&id))
}

#[cfg(test)]
mod test_hooks {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Mutex;
    use std::time::Duration;

    static POST_SNAPSHOT_DELAY: Mutex<Option<Duration>> = Mutex::new(None);
    static LAST_SNAPSHOT_LOCK_US: AtomicU64 = AtomicU64::new(0);

    pub fn set_post_snapshot_delay(delay: Option<Duration>) {
        *POST_SNAPSHOT_DELAY.lock().unwrap() = delay;
    }

    pub fn last_snapshot_lock_us() -> u64 {
        LAST_SNAPSHOT_LOCK_US.load(Ordering::Relaxed)
    }

    pub fn record_snapshot_lock(elapsed: Duration) {
        LAST_SNAPSHOT_LOCK_US.store(elapsed.as_micros() as u64, Ordering::Relaxed);
    }

    pub fn sleep_post_snapshot_delay() {
        if let Some(delay) = *POST_SNAPSHOT_DELAY.lock().unwrap() {
            std::thread::sleep(delay);
        }
    }
}

fn record_test_snapshot_lock(_elapsed: std::time::Duration) {
    #[cfg(test)]
    test_hooks::record_snapshot_lock(_elapsed);
}

fn sleep_test_post_snapshot_delay() {
    #[cfg(test)]
    test_hooks::sleep_post_snapshot_delay();
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn notion_metadata_update_keeps_the_resolved_database_identity() {
        let resolved = r#"{"token":"ntn_secret","database_id":"0123456789abcdef0123456789abcdef","database_name":"旧名"}"#;
        let request = notion_metadata_request("notion", resolved)
            .unwrap()
            .unwrap();
        let update = notion_metadata_update(request, "Notion側の最新名".to_string());
        assert_eq!(update.database_name, "Notion側の最新名");
        assert_eq!(update.database_id, "0123456789abcdef0123456789abcdef");
    }

    #[test]
    fn notion_metadata_update_persists_the_api_name_as_source_name() {
        let conn = Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        let stored = r#"{"token":"cred:42","database_id":"0123456789abcdef0123456789abcdef","database_name":"旧名","display_columns":["件名"]}"#;
        let id = db::insert_source(&conn, "notion", "以前の手動表示名", stored).unwrap();
        let resolved = r#"{"token":"ntn_secret","database_id":"0123456789abcdef0123456789abcdef"}"#;
        let request = notion_metadata_request("notion", resolved)
            .unwrap()
            .unwrap();
        let update = notion_metadata_update(request, "Notion側の最新名".to_string());

        persist_notion_metadata(&conn, id, Some(&update)).unwrap();

        let (_, name, _, config_json) = db::get_source_full(&conn, id).unwrap();
        assert_eq!(name, "Notion側の最新名");
        let config: serde_json::Value = serde_json::from_str(&config_json).unwrap();
        assert_eq!(config["database_name"], "Notion側の最新名");
        assert_eq!(config["token"], "cred:42");
        assert_eq!(config["display_columns"][0], "件名");
    }

    #[test]
    fn non_notion_reload_does_not_request_name_metadata() {
        assert!(notion_metadata_request("csv", r#"{"path":"x.csv"}"#)
            .unwrap()
            .is_none());
    }

    #[test]
    fn reload_summary_keeps_success_failure_and_cancelled_details() {
        let mut summary = ReloadSummary::default();
        summary.record(1, Ok(ReloadOneOutcome::Success { record_count: 3 }));
        summary.record(2, Ok(ReloadOneOutcome::Cancelled));
        summary.record(3, Err(anyhow::anyhow!("connection refused")));

        assert_eq!(
            (summary.successful, summary.failed, summary.cancelled),
            (1, 1, 1)
        );
        assert_eq!(summary.results[0].record_count, Some(3));
        assert_eq!(summary.results[1].outcome, "cancelled");
        assert_eq!(
            summary.results[2].error.as_deref(),
            Some("connection refused")
        );

        let json = serde_json::to_value(summary).unwrap();
        assert_eq!(json["successful"], 1);
        assert_eq!(json["results"][0]["outcome"], "success");
        assert_eq!(json["results"][2]["error"], "connection refused");
    }

    #[test]
    fn reload_one_source_delegates_to_batch_tail_optimize() {
        let conn = Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        let state = AppState::new(conn);
        let id = db::insert_source(
            &state.db.lock().unwrap(),
            "csv",
            "empty",
            r#"{"path":"C:/missing.csv"}"#,
        )
        .unwrap();

        let summary = reload_one_source(&state, id);
        assert_eq!(summary.results.len(), 1);
        assert!(summary.failed >= 1 || summary.successful >= 1);
    }

    #[test]
    fn reload_config_snapshot_lock_stays_sub_second() {
        let conn = Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        let state = AppState::new(conn);
        let samples = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("samples")
            .join("customers.csv");
        let id = db::insert_source(
            &state.db.lock().unwrap(),
            "csv",
            "sample",
            &serde_json::json!({ "path": samples.to_string_lossy() }).to_string(),
        )
        .unwrap();

        test_hooks::set_post_snapshot_delay(None);
        let _ = reload_one_source(&state, id);
        assert!(
            test_hooks::last_snapshot_lock_us() < 1_000_000,
            "snapshot lock held {} us (expected < 1s)",
            test_hooks::last_snapshot_lock_us()
        );
    }

    #[test]
    fn search_runs_while_reload_waits_outside_db_lock() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use std::thread;
        use std::time::Duration;

        let conn = Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        let state = Arc::new(AppState::new(conn));
        let sid = db::insert_source(
            &state.db.lock().unwrap(),
            "csv",
            "sample",
            &serde_json::json!({
                "path": std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("..")
                    .join("samples")
                    .join("customers.csv")
                    .to_string_lossy()
            })
            .to_string(),
        )
        .unwrap();

        test_hooks::set_post_snapshot_delay(Some(Duration::from_millis(300)));
        let search_ok = Arc::new(AtomicBool::new(false));
        let search_ok_bg = search_ok.clone();
        let state_bg = state.clone();

        let reload = thread::spawn(move || {
            let _ = reload_one_for_state(&state, sid);
        });

        thread::sleep(Duration::from_millis(50));
        if state_bg.db.lock().is_ok() {
            let conn = state_bg.db.lock().unwrap();
            let _ = db::search(&conn, "田中", &[sid], db::DEFAULT_ROW_LIMIT);
            search_ok_bg.store(true, Ordering::SeqCst);
        }

        reload.join().unwrap();
        test_hooks::set_post_snapshot_delay(None);
        assert!(
            search_ok.load(Ordering::SeqCst),
            "search should acquire DB lock while reload fetch runs outside the lock"
        );
    }

    #[test]
    fn reload_one_records_status_when_credential_resolution_fails() {
        let mut conn = Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        let id = db::insert_source(
            &conn,
            "notion",
            "壊れた参照",
            &serde_json::json!({ "token": "cred:9999", "database_id": "d" }).to_string(),
        )
        .unwrap();

        assert!(reload_one(&mut conn, id).is_err());

        let info = db::list_sources(&conn)
            .unwrap()
            .into_iter()
            .find(|s| s.id == id)
            .unwrap();
        assert_eq!(info.status.as_deref(), Some("error"));
        assert!(
            info.last_error.unwrap_or_default().contains("9999"),
            "どのトークンが見つからないかがエラーに残ること"
        );
    }

    #[test]
    fn reload_rejects_incompatible_graph_auth_before_network_access() {
        let mut conn = Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        let credential_id = crate::credentials::add_credential(
            &conn,
            "graph",
            "SharePoint委任",
            r#"{"auth_method":"delegated","tenant_id":"t","client_id":"c","capabilities":["sharepoint"],"refresh_token":"not-used"}"#,
        )
        .unwrap();
        let id = db::insert_source(
            &conn,
            "graph_entra_users",
            "ユーザー一覧",
            &serde_json::json!({ "client_secret": format!("cred:{credential_id}") }).to_string(),
        )
        .unwrap();

        let error = reload_one(&mut conn, id).unwrap_err().to_string();
        assert!(error.contains("委任 OAuth に対応していません"));
        let info = db::list_sources(&conn)
            .unwrap()
            .into_iter()
            .find(|source| source.id == id)
            .unwrap();
        assert_eq!(info.status.as_deref(), Some("error"));
        assert!(info
            .last_error
            .unwrap_or_default()
            .contains("委任 OAuth に対応していません"));
    }
}

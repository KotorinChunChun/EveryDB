//! データソースの再取得（スナップショット全置換）と定期更新スケジューラ。

use std::time::Duration;

use rusqlite::Connection;
use tauri::{AppHandle, Emitter, Manager};

use crate::app_state::AppState;
use crate::{connectors, credentials, db};

/// 1ソースのリロード結果。既に同一sourceを更新中の場合は、重複実行せず取消扱いにする。
#[derive(Debug, PartialEq, Eq)]
pub enum ReloadOneOutcome {
    Success { record_count: usize },
    Cancelled,
}

/// IPC・トレイ・schedulerで共通利用する、ソース単位のリロード結果。
#[derive(serde::Serialize)]
pub struct ReloadSourceResult {
    pub source_id: i64,
    /// `success` / `failed` / `cancelled`
    pub outcome: &'static str,
    pub record_count: Option<usize>,
    pub error: Option<String>,
}

/// まとめてリロードした結果。個別の失敗は全体失敗にせず、呼び出し元へ明示的に返す。
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
}

/// 1 ソースを取得してスナップショット全置換し、ステータスを更新する（要件 §4-1）。
pub fn reload_one(conn: &mut Connection, id: i64) -> anyhow::Result<()> {
    let (type_, cfg) = db::get_source_config(conn, id)?;
    // 取得前段（保存済みトークンの解決・コネクタ構築）の失敗もステータスへ必ず記録する。
    // ここを素通しすると、呼び出し元の reload_ids が `let _ =` で握り潰すため、
    // 例えば Box の認可切れ（refresh_token 失効）がユーザーに一切見えなくなる。
    let prepared = (|| -> anyhow::Result<Box<dyn connectors::Connector>> {
        // 保存済みトークンへの参照（cred:<id>）を実値へ展開してから接続する。
        // DB に保存される config_json 自体（cfg）は変更しない＝参照は維持される。
        let resolved_cfg = credentials::resolve_credentials(conn, &cfg)?;
        connectors::build_connector(&type_, &resolved_cfg)
    })();
    let connector = match prepared {
        Ok(c) => c,
        Err(e) => {
            let _ = db::set_status(conn, id, "error", Some(&e.to_string()));
            return Err(e);
        }
    };
    match connector.fetch() {
        Ok(records) => {
            db::replace_source_records(conn, id, &records)?;
            db::set_status(conn, id, "ok", None)?;
            Ok(())
        }
        Err(e) => {
            let _ = db::set_status(conn, id, "error", Some(&e.to_string()));
            Err(e)
        }
    }
}

/// AppState経由で1ソースを再取得する。DBロックは設定取得・結果保存の短時間だけ保持し、
/// connector.fetch() によるファイル/ネットワークI/Oは必ずロック外で実行する。
pub fn reload_one_for_state(state: &AppState, id: i64) -> anyhow::Result<ReloadOneOutcome> {
    let _reload_guard = match state.try_begin_reload(id) {
        Some(guard) => guard,
        None => return Ok(ReloadOneOutcome::Cancelled), // 同じsourceの更新中。既存の処理を待たせず重複取得を避ける。
    };
    let connector = {
        let conn = state
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("lock poisoned"))?;
        let prepared = (|| -> anyhow::Result<Box<dyn connectors::Connector>> {
            let (type_, cfg) = db::get_source_config(&conn, id)?;
            let resolved_cfg = credentials::resolve_credentials(&conn, &cfg)?;
            connectors::build_connector(&type_, &resolved_cfg)
        })();
        match prepared {
            Ok(connector) => connector,
            Err(error) => {
                let _ = db::set_status(&conn, id, "error", Some(&error.to_string()));
                return Err(error);
            }
        }
    };

    let fetched = connector.fetch();
    let mut conn = state
        .db
        .lock()
        .map_err(|_| anyhow::anyhow!("lock poisoned"))?;
    match fetched {
        Ok(records) => {
            db::replace_source_records(&mut conn, id, &records)?;
            db::set_status(&conn, id, "ok", None)?;
            Ok(ReloadOneOutcome::Success {
                record_count: records.len(),
            })
        }
        Err(error) => {
            let _ = db::set_status(&conn, id, "error", Some(&error.to_string()));
            Err(error)
        }
    }
}

/// FTS最適化を呼び出し元の更新単位ごとに1回だけ行う。
pub fn optimize_fts(state: &AppState) {
    if let Ok(conn) = state.db.lock() {
        let _ = conn.execute(
            "INSERT INTO records_fts(records_fts) VALUES('optimize')",
            [],
        );
    }
}
/// 指定 id 群を再取得する。個別失敗はstatusへ記録したうえで、呼び出し元へ集計結果として返す。
pub fn reload_many_for_state(state: &AppState, ids: &[i64]) -> ReloadSummary {
    let mut summary = ReloadSummary::default();
    for &id in ids {
        summary.record(id, reload_one_for_state(state, id));
    }
    optimize_fts(state);
    summary
}

/// 指定 id 群を再取得し、完了後にフロントへ通知する。トレイ・schedulerは結果を表示しないが、
/// 同じ集計契約を利用する。
pub fn reload_ids(app: &AppHandle, ids: &[i64]) -> ReloadSummary {
    let summary = {
        let state = app.state::<AppState>();
        reload_many_for_state(&state, ids)
    };
    let _ = app.emit("sources-updated", ());
    summary
}

/// 全ソースを今すぐ更新（トレイメニュー用）。
pub fn reload_all_now(app: &AppHandle) {
    let ids = {
        let state = app.state::<AppState>();
        let conn = match state.db.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        db::list_sources(&conn)
            .map(|v| v.into_iter().map(|s| s.id).collect::<Vec<_>>())
            .unwrap_or_default()
    };
    reload_ids(app, &ids);
}

/// refresh_interval を過ぎた（または未更新の）ソース id を返す。
fn due_ids(app: &AppHandle) -> Vec<i64> {
    let state = app.state::<AppState>();
    let conn = match state.db.lock() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let mut stmt = match conn.prepare(
        "SELECT id FROM sources
         WHERE last_success_at IS NULL
            OR datetime(last_success_at) <= datetime('now','localtime','-' || refresh_interval || ' seconds')",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let ids = stmt
        .query_map([], |r| r.get::<_, i64>(0))
        .map(|rows| rows.flatten().collect::<Vec<_>>())
        .unwrap_or_default();
    ids
}

/// 内部タイマー。60 秒ごとに期限到来ソースを更新する（要件 §2 常駐 + §4-1）。
pub fn spawn_scheduler(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(60));
        let due = due_ids(&app);
        if !due.is_empty() {
            reload_ids(&app, &due);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

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
    /// 取得前段（トークン解決）で失敗したときも status に error を残すこと。
    /// ここを素通りすると reload_ids が握り潰すため、Box の認可切れ等が UI に出なくなる。
    #[test]
    fn reload_one_records_status_when_credential_resolution_fails() {
        let mut conn = Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        // 存在しない保存済みトークンを参照するソース（＝解決が必ず失敗する）。
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
}

//! データソースの再取得（スナップショット全置換）と定期更新スケジューラ。

use std::time::Duration;

use rusqlite::Connection;
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::AppState;
use crate::{connectors, credentials, db};

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

/// 指定 id 群を再取得し、完了後にフロントへ通知する。個別失敗は status に記録して続行。
pub fn reload_ids(app: &AppHandle, ids: &[i64]) {
    {
        let state = app.state::<AppState>();
        let mut conn = match state.db.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        for &id in ids {
            let _ = reload_one(&mut conn, id);
        }
        // リロード完了時に FTS5 インデックスを最適化（断片化解消による高速化）
        let _ = conn.execute(
            "INSERT INTO records_fts(records_fts) VALUES('optimize')",
            [],
        );
    }
    let _ = app.emit("sources-updated", ());
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

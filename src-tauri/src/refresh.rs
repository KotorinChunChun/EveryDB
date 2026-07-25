//! データソースの再取得（スナップショット全置換）と定期更新スケジューラ。

use std::time::Duration;

use rusqlite::Connection;
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::AppState;
use crate::{connectors, db};

/// 1 ソースを取得してスナップショット全置換し、ステータスを更新する（要件 §4-1）。
pub fn reload_one(conn: &mut Connection, id: i64) -> anyhow::Result<()> {
    let (type_, cfg) = db::get_source_config(conn, id)?;
    let connector = connectors::build_connector(&type_, &cfg)?;
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
        db::list_sources(&conn).map(|v| v.into_iter().map(|s| s.id).collect::<Vec<_>>()).unwrap_or_default()
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

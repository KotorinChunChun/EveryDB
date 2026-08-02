//! Tauri 連携付きの再取得スケジューラ（emit / トレイ / 定期更新）。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::app_state::AppState;
use crate::db;
use crate::internal_log;

pub use crate::refresh_service::{
    optimize_fts, reload_many_for_state, reload_one, reload_one_for_state, ReloadOneOutcome,
    ReloadSourceResult, ReloadSummary,
};

pub fn reload_ids(app: &AppHandle, ids: &[i64]) -> ReloadSummary {
    let summary = {
        let state = app.state::<AppState>();
        reload_many_for_state(&state, ids)
    };
    if let Err(error) = app.emit("sources-updated", ()) {
        internal_log::warn("reload", format!("sources-updated emit failed: {error}"));
    }
    summary
}

pub fn reload_all_now(app: &AppHandle) {
    let ids = {
        let state = app.state::<AppState>();
        let conn = match state.db.lock() {
            Ok(c) => c,
            Err(_) => {
                internal_log::warn("reload", "DB lock poisoned while listing sources");
                return;
            }
        };
        db::list_sources(&conn)
            .map(|v| v.into_iter().map(|s| s.id).collect::<Vec<_>>())
            .unwrap_or_default()
    };
    reload_ids(app, &ids);
}

fn due_ids(app: &AppHandle) -> Vec<i64> {
    let state = app.state::<AppState>();
    let conn = match state.db.lock() {
        Ok(c) => c,
        Err(_) => {
            internal_log::warn("scheduler", "DB lock poisoned while checking due sources");
            return Vec::new();
        }
    };
    let mut stmt = match conn.prepare(
        "SELECT id FROM sources
         WHERE last_success_at IS NULL
            OR datetime(last_success_at) <= datetime('localtime','-' || refresh_interval || ' seconds')",
    ) {
        Ok(s) => s,
        Err(error) => {
            internal_log::warn("scheduler", format!("due query prepare failed: {error}"));
            return Vec::new();
        }
    };
    stmt.query_map([], |r| r.get::<_, i64>(0))
        .map(|rows| rows.flatten().collect::<Vec<_>>())
        .unwrap_or_default()
}

pub fn spawn_scheduler(app: AppHandle, shutdown: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        while !shutdown.load(Ordering::Relaxed) {
            for _ in 0..60 {
                if shutdown.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
            if shutdown.load(Ordering::Relaxed) {
                return;
            }
            let due = due_ids(&app);
            if !due.is_empty() {
                reload_ids(&app, &due);
            }
        }
    });
}

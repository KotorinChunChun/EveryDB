//! アプリ終了・ウィンドウ操作時の後片付け。

use std::sync::atomic::Ordering;

use tauri::{AppHandle, Manager};

use crate::app_state::AppState;
use crate::credential_service;

/// 実行中 OAuth の認可待ちだけを取り消す。
///
/// ウィンドウをトレイへ格納するときに使用し、定期 reload は継続させる。
pub fn cancel_oauth_tasks(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    cancel_oauth_tasks_for_state(&state);
}

/// プロセス終了時に定期 reload スケジューラを停止し、実行中 OAuth も取り消す。
pub fn shutdown_background_tasks(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    shutdown_background_tasks_for_state(&state);
}

fn cancel_oauth_tasks_for_state(state: &AppState) {
    credential_service::cancel_box_oauth_on_shutdown(state);
    credential_service::cancel_graph_oauth_on_shutdown(state);
}

fn shutdown_background_tasks_for_state(state: &AppState) {
    state.scheduler_shutdown.store(true, Ordering::Relaxed);
    cancel_oauth_tasks_for_state(state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn tray_hide_cancels_oauth_without_stopping_scheduler() {
        let state = AppState::new(Connection::open_in_memory().unwrap());

        cancel_oauth_tasks_for_state(&state);

        assert!(!state.scheduler_shutdown.load(Ordering::Relaxed));
    }

    #[test]
    fn process_exit_stops_scheduler() {
        let state = AppState::new(Connection::open_in_memory().unwrap());

        shutdown_background_tasks_for_state(&state);

        assert!(state.scheduler_shutdown.load(Ordering::Relaxed));
    }
}

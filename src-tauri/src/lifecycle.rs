//! アプリ終了・ウィンドウ操作時の後片付け。

use std::sync::atomic::Ordering;

use tauri::{AppHandle, Manager};

use crate::app_state::AppState;
use crate::credential_service;

/// 定期 reload スケジューラ停止と実行中 Box OAuth の取消。
pub fn shutdown_background_tasks(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    state.scheduler_shutdown.store(true, Ordering::Relaxed);
    credential_service::cancel_box_oauth_on_shutdown(&state);
}

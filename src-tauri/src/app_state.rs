//! Tauriコマンドが共有するアプリケーション状態。

use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rusqlite::Connection;

const UNLOCK_DURATION: Duration = Duration::from_secs(600); // 直近の Windows Hello 確認から 10 分は再要求しない

pub struct AppState {
    pub db: Mutex<Connection>,
    /// 直近の Windows Hello 確認の有効期限（None または過去時刻 = 未確認/期限切れ）。
    unlocked_until: Mutex<Option<Instant>>,
    /// 実行中の Box OAuth 認可待ちの取消フラグ（同時に1件だけ想定。§7-19）。
    pub(crate) box_oauth_cancel: Mutex<Option<Arc<AtomicBool>>>,
    /// 実行中の Microsoft Graph 委任 OAuth 認可待ちの取消フラグ。
    pub(crate) graph_oauth_cancel: Mutex<Option<Arc<AtomicBool>>>,
    /// 同一sourceに対する重複reloadを抑止するための実行中ID。
    reloading_source_ids: Mutex<HashSet<i64>>,
    /// 定期 reload スレッドへ終了を通知するフラグ。
    pub(crate) scheduler_shutdown: Arc<AtomicBool>,
}

impl AppState {
    pub fn new(conn: Connection) -> Self {
        Self {
            db: Mutex::new(conn),
            unlocked_until: Mutex::new(None),
            box_oauth_cancel: Mutex::new(None),
            graph_oauth_cancel: Mutex::new(None),
            reloading_source_ids: Mutex::new(HashSet::new()),
            scheduler_shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_unlocked(&self) -> bool {
        self.unlocked_until
            .lock()
            .map(|g| g.map(|t| Instant::now() < t).unwrap_or(false))
            .unwrap_or(false)
    }

    /// sourceが未実行ならreload枠を確保する。戻り値をdropすると必ず枠を解放する。
    pub(crate) fn try_begin_reload(&self, source_id: i64) -> Option<ReloadGuard<'_>> {
        let mut ids = self.reloading_source_ids.lock().ok()?;
        if !ids.insert(source_id) {
            return None;
        }
        Some(ReloadGuard {
            state: self,
            source_id,
        })
    }

    pub(crate) fn unlock(&self) {
        if let Ok(mut g) = self.unlocked_until.lock() {
            *g = Some(Instant::now() + UNLOCK_DURATION);
        }
    }
}

pub(crate) struct ReloadGuard<'a> {
    state: &'a AppState,
    source_id: i64,
}

impl Drop for ReloadGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut ids) = self.state.reloading_source_ids.lock() {
            ids.remove(&self.source_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reload_guard_suppresses_only_the_same_source_until_released() {
        let state = AppState::new(Connection::open_in_memory().unwrap());
        let first = state
            .try_begin_reload(10)
            .expect("first reload must acquire the guard");
        assert!(state.try_begin_reload(10).is_none());
        assert!(state.try_begin_reload(11).is_some());
        drop(first);
        assert!(state.try_begin_reload(10).is_some());
    }
}

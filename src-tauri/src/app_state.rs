//! Tauriコマンドが共有するアプリケーション状態。

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
}

impl AppState {
    pub fn new(conn: Connection) -> Self {
        Self {
            db: Mutex::new(conn),
            unlocked_until: Mutex::new(None),
            box_oauth_cancel: Mutex::new(None),
        }
    }

    pub fn is_unlocked(&self) -> bool {
        self.unlocked_until
            .lock()
            .map(|g| g.map(|t| Instant::now() < t).unwrap_or(false))
            .unwrap_or(false)
    }

    pub(crate) fn unlock(&self) {
        if let Ok(mut g) = self.unlocked_until.lock() {
            *g = Some(Instant::now() + UNLOCK_DURATION);
        }
    }
}

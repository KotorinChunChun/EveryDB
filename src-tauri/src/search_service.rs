//! 検索・検索履歴・検索プリセットの業務処理。
//!
//! Tauri command 型に依存せず、AppState とDBの境界だけを扱う。

use crate::app_state::AppState;
use crate::db;
use crate::model::{SearchPreset, SearchResponse};
use crate::service_error::{lock_db, ServiceError, ServiceResult};

pub fn search(
    state: &AppState,
    query: &str,
    source_ids: &[i64],
    seq: i64,
    limit: Option<i64>,
) -> ServiceResult<SearchResponse> {
    let conn = lock_db(state)?;
    let limit = limit.unwrap_or(db::DEFAULT_ROW_LIMIT);
    let (rows, truncated) = db::search(&conn, query, source_ids, limit)
        .map_err(|e| ServiceError::external("検索に失敗しました", e))?;
    Ok(SearchResponse {
        seq,
        rows,
        truncated,
    })
}

pub fn add_history(state: &AppState, query: &str) -> ServiceResult<()> {
    let conn = lock_db(state)?;
    db::add_search_history(&conn, query)
        .map_err(|e| ServiceError::external("検索履歴の保存に失敗しました", e))
}

pub fn list_history(state: &AppState) -> ServiceResult<Vec<String>> {
    let conn = lock_db(state)?;
    db::list_search_history(&conn, 15)
        .map_err(|e| ServiceError::external("検索履歴の取得に失敗しました", e))
}

pub fn save_preset(
    state: &AppState,
    name: &str,
    query: &str,
    source_ids: &[i64],
) -> ServiceResult<i64> {
    let conn = lock_db(state)?;
    db::save_search_preset(&conn, name, query, source_ids)
        .map_err(|e| ServiceError::external("お気に入りの保存に失敗しました", e))
}

pub fn list_presets(state: &AppState) -> ServiceResult<Vec<SearchPreset>> {
    let conn = lock_db(state)?;
    db::list_search_presets(&conn)
        .map_err(|e| ServiceError::external("お気に入り一覧の取得に失敗しました", e))
}

pub fn delete_preset(state: &AppState, id: i64) -> ServiceResult<()> {
    let conn = lock_db(state)?;
    db::delete_search_preset(&conn, id)
        .map_err(|e| ServiceError::external("お気に入りの削除に失敗しました", e))
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{add_history, list_history, search};
    use crate::app_state::AppState;
    use crate::db;

    fn test_state() -> AppState {
        let conn = Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        AppState::new(conn)
    }

    #[test]
    fn service_keeps_history_and_preserves_search_sequence() {
        let state = test_state();
        add_history(&state, "  customer  ").unwrap();
        assert_eq!(list_history(&state).unwrap(), vec!["customer"]);

        let response = search(&state, "customer", &[], 42, None).unwrap();
        assert_eq!(response.seq, 42);
        assert!(response.rows.is_empty());
        assert!(!response.truncated);
    }
}

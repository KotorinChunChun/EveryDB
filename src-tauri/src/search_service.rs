//! 検索・検索履歴・検索プリセットの業務処理。
//!
//! Tauri command 型に依存せず、AppState とDBの境界だけを扱う。

use crate::app_state::AppState;
use crate::db;
use crate::model::{SearchPreset, SearchResponse};

fn err_to_string<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

pub fn search(
    state: &AppState,
    query: &str,
    source_ids: &[i64],
    seq: i64,
    limit: Option<i64>,
) -> Result<SearchResponse, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let limit = limit.unwrap_or(db::DEFAULT_ROW_LIMIT);
    let (rows, truncated) = db::search(&conn, query, source_ids, limit).map_err(err_to_string)?;
    Ok(SearchResponse {
        seq,
        rows,
        truncated,
    })
}

pub fn add_history(state: &AppState, query: &str) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::add_search_history(&conn, query).map_err(err_to_string)
}

pub fn list_history(state: &AppState) -> Result<Vec<String>, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::list_search_history(&conn, 15).map_err(err_to_string)
}

pub fn save_preset(
    state: &AppState,
    name: &str,
    query: &str,
    source_ids: &[i64],
) -> Result<i64, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::save_search_preset(&conn, name, query, source_ids).map_err(err_to_string)
}

pub fn list_presets(state: &AppState) -> Result<Vec<SearchPreset>, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::list_search_presets(&conn).map_err(err_to_string)
}

pub fn delete_preset(state: &AppState, id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::delete_search_preset(&conn, id).map_err(err_to_string)
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

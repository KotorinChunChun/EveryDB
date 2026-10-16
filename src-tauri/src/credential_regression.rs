//! RF-60-06: credential 削除・置換・参照切れ・Box OAuth/CCG の回帰テスト。

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use serde_json::Value;

    use crate::credentials;
    use crate::db;

    fn open_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        conn
    }

    fn insert_notion_source(conn: &Connection, name: &str, cred_id: i64) -> i64 {
        db::insert_source(
            conn,
            "notion",
            name,
            &serde_json::json!({
                "token": format!("cred:{cred_id}"),
                "database_id": "db-1"
            })
            .to_string(),
        )
        .unwrap()
    }

    #[test]
    fn replace_credential_propagates_to_all_linked_sources() {
        let conn = open_test_db();
        let id = credentials::add_credential(&conn, "notion", "shared", r#"{"token":"ntn_v1"}"#)
            .unwrap();
        insert_notion_source(&conn, "ソースA", id);
        insert_notion_source(&conn, "ソースB", id);

        credentials::update_credential(&conn, id, "shared", r#"{"token":"ntn_v2"}"#).unwrap();

        let cfg =
            serde_json::json!({ "token": format!("cred:{id}"), "database_id": "db-1" }).to_string();
        let resolved = credentials::resolve_credentials(&conn, &cfg).unwrap();
        let v: Value = serde_json::from_str(&resolved).unwrap();
        assert_eq!(v["token"], "ntn_v2");
        let list = credentials::list_credentials(&conn, "notion").unwrap();
        assert_eq!(list[0].used_by.len(), 2);
    }

    #[test]
    fn delete_source_then_delete_credential_succeeds() {
        let conn = open_test_db();
        let id = credentials::add_credential(&conn, "notion", "temp", r#"{"token":"ntn_temp"}"#)
            .unwrap();
        let sid = insert_notion_source(&conn, "一時DB", id);

        db::delete_source(&conn, sid).unwrap();
        credentials::delete_credential(&conn, id).unwrap();
        assert!(credentials::list_credentials(&conn, "notion")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn delete_credential_lists_every_using_source() {
        let conn = open_test_db();
        let id =
            credentials::add_credential(&conn, "notion", "multi", r#"{"token":"ntn_x"}"#).unwrap();
        insert_notion_source(&conn, "Alpha", id);
        insert_notion_source(&conn, "Beta", id);

        let err = credentials::delete_credential(&conn, id).unwrap_err();
        assert!(err.contains("2件"));
        assert!(err.contains("Alpha"));
        assert!(err.contains("Beta"));
    }

    #[test]
    fn missing_credential_ref_errors_on_resolve_with_id() {
        let conn = open_test_db();
        let cfg = serde_json::json!({ "token": "cred:4242", "database_id": "db" }).to_string();
        let err = credentials::resolve_credentials(&conn, &cfg).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("4242"));
        assert!(msg.contains("見つかりません"));
    }

    #[test]
    fn box_ccg_missing_enterprise_id_fails_before_network() {
        let conn = open_test_db();
        let id = credentials::add_credential(
            &conn,
            "box",
            "enterprise",
            r#"{"auth_method":"ccg","client_id":"cid","client_secret":"csec"}"#,
        )
        .unwrap();
        let cfg = serde_json::json!({ "access_token": format!("cred:{id}"), "folder_id": "0" })
            .to_string();
        let err = credentials::resolve_credentials(&conn, &cfg).unwrap_err();
        assert!(
            err.to_string().contains("Enterprise ID"),
            "CCG validation must fail locally: {err}"
        );
    }

    #[test]
    fn box_oauth_missing_refresh_fails_before_network() {
        let conn = open_test_db();
        let id = credentials::add_credential(
            &conn,
            "box",
            "personal",
            r#"{"client_id":"cid","client_secret":"csec"}"#,
        )
        .unwrap();
        let cfg = serde_json::json!({ "access_token": format!("cred:{id}"), "folder_id": "0" })
            .to_string();
        let err = credentials::resolve_credentials(&conn, &cfg).unwrap_err();
        assert!(
            err.to_string().contains("認可"),
            "OAuth path must fail clearly without refresh_token: {err}"
        );
    }
}

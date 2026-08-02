//! RF-60-05: 秘密値が log / IPC / フロント state へ漏れないことの回帰テスト。

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use crate::app_state::AppState;
    use crate::credentials;
    use crate::db;
    use crate::secret::{self, SECRET_PLACEHOLDER};
    use crate::source_service;

    #[test]
    fn locked_source_detail_masks_dpapi_and_cred_refs() {
        let conn = Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        let protected = secret::protect_config(
            "notion",
            r#"{"token":"ntn_super_secret","database_id":"db-1"}"#,
        )
        .unwrap();
        assert!(protected.contains("dpapi:"));
        let id = db::insert_source(&conn, "notion", "locked", &protected).unwrap();
        let state = AppState::new(conn);

        let detail = source_service::get_source_detail(&state, id).unwrap();
        let config = detail.get("config").expect("config object");
        assert_eq!(
            config.get("token").and_then(|v| v.as_str()),
            Some(SECRET_PLACEHOLDER)
        );
        assert_eq!(
            detail.get("unlocked").and_then(|v| v.as_bool()),
            Some(false)
        );
        let serialized = detail.to_string();
        assert!(!serialized.contains("ntn_super_secret"));
        assert!(!serialized.contains("dpapi:"));
    }

    #[test]
    fn unlocked_source_detail_still_masks_cred_linked_secret_field() {
        let conn = Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        let cred_id =
            credentials::add_credential(&conn, "notion", "prod", r#"{"token":"ntn_from_vault"}"#)
                .unwrap();
        let config = format!(r#"{{"token":"cred:{cred_id}","database_id":"db-1"}}"#);
        let id = db::insert_source(&conn, "notion", "linked", &config).unwrap();
        let state = AppState::new(conn);
        state.unlock();

        let detail = source_service::get_source_detail(&state, id).unwrap();
        let token = detail
            .get("config")
            .and_then(|c| c.get("token"))
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert_eq!(token, SECRET_PLACEHOLDER);
        assert!(!detail.to_string().contains("ntn_from_vault"));
    }

    #[test]
    fn credential_public_fields_never_include_secret_values() {
        let conn = Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        let id = credentials::add_credential(
            &conn,
            "graph",
            "tenant app",
            r#"{"tenant_id":"t1","client_id":"cid","client_secret":"graph-secret-value"}"#,
        )
        .unwrap();
        let (label, fields) = credentials::public_fields(&conn, id).unwrap();
        assert_eq!(label, "tenant app");
        assert_eq!(fields.get("tenant_id").and_then(|v| v.as_str()), Some("t1"));
        assert!(!fields.contains_key("client_secret"));
        let fields_json = serde_json::to_string(&fields).unwrap();
        assert!(!fields_json.contains("graph-secret-value"));
    }

    #[test]
    fn reload_error_references_missing_cred_id_not_secret_payload() {
        let mut conn = Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        credentials::add_credential(&conn, "notion", "real", r#"{"token":"ntn_real"}"#).unwrap();
        let id = db::insert_source(
            &conn,
            "notion",
            "broken link",
            r#"{"token":"cred:4242","database_id":"db"}"#,
        )
        .unwrap();

        let err = crate::refresh_service::reload_one(&mut conn, id)
            .unwrap_err()
            .to_string();
        assert!(err.contains("4242"), "missing cred id may appear in errors");
        assert!(!err.contains("ntn_real"), "unrelated secret must not leak");
        assert!(!err.contains("dpapi:"));
    }
}

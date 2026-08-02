//! RF-60-08: SQL / FTS / LIKE / identifier / 取込データの入力境界監査テスト。

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use serde_json::json;

    use crate::db;
    use crate::model::Record;

    fn open_with_records(rows: &[(&str, &str)]) -> (Connection, i64) {
        let mut conn = Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        let sid = db::insert_source(&conn, "csv", "boundary", "{}").unwrap();
        let recs: Vec<Record> = rows
            .iter()
            .map(|(label, text)| {
                let mut fields = serde_json::Map::new();
                fields.insert("note".into(), json!(text));
                Record {
                    label: Some((*label).into()),
                    open_ref: None,
                    row_number: None,
                    fields,
                    search_text: (*text).to_string(),
                }
            })
            .collect();
        db::replace_source_records(&mut conn, sid, &recs).unwrap();
        conn.execute(
            "INSERT INTO records_fts(records_fts) VALUES('optimize')",
            [],
        )
        .unwrap();
        (conn, sid)
    }

    #[test]
    fn like_query_percent_is_literal_not_wildcard() {
        let (conn, sid) = open_with_records(&[
            ("plain", "alpha beta"),
            ("percent", "progress 100% complete"),
        ]);
        let (hits, _) = db::search(&conn, "100%", &[sid], db::DEFAULT_ROW_LIMIT).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].label.as_deref(), Some("percent"));
        let (none, _) = db::search(&conn, "%complete", &[sid], db::DEFAULT_ROW_LIMIT).unwrap();
        assert!(
            none.is_empty(),
            "leading % must not act as LIKE wildcard: got {} hits",
            none.len()
        );
    }

    #[test]
    fn like_query_underscore_is_literal() {
        let (conn, sid) =
            open_with_records(&[("underscore", "file_name_v2"), ("plain", "filename v2")]);
        let (hits, _) = db::search(&conn, "file_name", &[sid], db::DEFAULT_ROW_LIMIT).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].label.as_deref(), Some("underscore"));
    }

    #[test]
    fn like_query_backslash_escape_is_honored() {
        let (conn, sid) = open_with_records(&[("escaped", r"path\to\share")]);
        let (hits, _) = db::search(&conn, r"path\to", &[sid], db::DEFAULT_ROW_LIMIT).unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn fts_query_embedded_double_quote_does_not_return_all_rows() {
        let (conn, sid) =
            open_with_records(&[("quoted", r#""special" token"#), ("other", "ordinary text")]);
        let (hits, _) = db::search(&conn, r#""special""#, &[sid], db::DEFAULT_ROW_LIMIT).unwrap();
        assert!(
            hits.len() <= 1,
            "FTS phrase with quotes must not broaden to all rows: {} hits",
            hits.len()
        );
    }

    #[test]
    fn multi_term_and_search_treats_each_term_literally() {
        let (conn, sid) = open_with_records(&[("both", "営業部 東京"), ("partial", "営業部 大阪")]);
        let (hits, _) = db::search(&conn, "営業部 東京", &[sid], db::DEFAULT_ROW_LIMIT).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].label.as_deref(), Some("both"));
    }

    #[test]
    fn empty_and_whitespace_only_queries_browse_without_error() {
        let (conn, sid) = open_with_records(&[("a", "one"), ("b", "two")]);
        let (all, _) = db::search(&conn, "", &[sid], db::DEFAULT_ROW_LIMIT).unwrap();
        assert_eq!(all.len(), 2);
        let (all, _) = db::search(&conn, "   ", &[sid], db::DEFAULT_ROW_LIMIT).unwrap();
        assert_eq!(all.len(), 2);
    }
}

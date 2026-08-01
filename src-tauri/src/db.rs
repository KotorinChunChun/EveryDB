use anyhow::{Context, Result};
use rusqlite::{params, params_from_iter, types::Value as SqlValue, Connection};
use serde_json::Map;
#[cfg(test)]
use std::path::Path;

use crate::model::{Record, Row};

const PER_SOURCE_CAP: i64 = 200;
/// 行数上限の既定値（UI から変更可能）。
pub const DEFAULT_ROW_LIMIT: i64 = 10000;
mod migrations;
pub use migrations::{init_schema, open_cached_database};
#[cfg(test)]
use migrations::{initialize_database_with, table_has_column, SCHEMA_VERSION_KEY};
mod sources;
#[cfg(test)]
use sources::source_parent;
pub use sources::{
    cached_field_names, delete_meta, delete_source, duplicate_source, get_meta, get_source_config,
    get_source_full, insert_source, list_parent_labels, list_sources, reorder_sources, set_meta,
    set_parent_label, set_refresh_interval, set_status, update_source,
};
/// スナップショット全置換（要件 §4-1）。トランザクション内で全削除 → 再挿入。
pub fn replace_source_records(
    conn: &mut Connection,
    source_id: i64,
    records: &[Record],
) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM records WHERE source_id = ?1", [source_id])?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO records(source_id, label, open_ref, row_number, fields_json, search_text)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for r in records {
            let fields_json = serde_json::to_string(&r.fields)?;
            stmt.execute(params![
                source_id,
                r.label,
                r.open_ref,
                r.row_number,
                fields_json,
                r.search_text
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// 検索（要件 §5-2 / §6-2 / §9-5）。
/// 空クエリはブラウズ、単一語は 1〜2 文字 LIKE / 3 文字以上 FTS、
/// **半角スペース区切りの複数語は AND 検索**（全語を LIKE で AND 結合、要件 §9-5）。
pub fn search(
    conn: &Connection,
    query: &str,
    source_ids: &[i64],
    limit: i64,
) -> Result<(Vec<Row>, bool)> {
    let limit = limit.clamp(1, 1_000_000); // 行数上限（要件: ユーザー設定可能）

    // データソースのチェックが全て外れている場合は「全件対象」ではなく 0 件を返す
    // （要件: 全チェックが外れているときは 1 件も出さない）。初回起動時は全ソースが
    // 明示的に選択されるため、空集合はユーザーが意図的に全て外した状態だけを意味する。
    if source_ids.is_empty() {
        return Ok((Vec::new(), false));
    }

    let terms: Vec<&str> = query
        .split(' ')
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect();

    let ph = vec!["?"; source_ids.len()].join(",");
    let in_clause = format!("r.source_id IN ({ph})");
    let id_params: Vec<SqlValue> = source_ids.iter().map(|&i| SqlValue::Integer(i)).collect();

    let rows = match terms.as_slice() {
        [] => browse(conn, &in_clause, &id_params, limit)?,
        [single] if single.chars().count() < 3 => {
            like_search(conn, single, &in_clause, &id_params, limit)?
        }
        [single] => fts_search(conn, single, &in_clause, &id_params, limit)?,
        multiple => multi_term_and_search(conn, multiple, &in_clause, &id_params, limit)?,
    };

    let truncated = rows.len() as i64 >= limit;
    Ok((rows, truncated))
}

/// 複数語 AND 検索。全語を `search_text LIKE` の AND で結合する
/// （語ごとに FTS/LIKE を混在させると MATCH の複数回結合が煩雑になるため、
/// 複数語ケースは LIKE の AND に統一する。対象件数は TOTAL_CAP で頭打ちのため十分高速）。
fn multi_term_and_search(
    conn: &Connection,
    terms: &[&str],
    in_clause: &str,
    id_params: &[SqlValue],
    limit: i64,
) -> Result<Vec<Row>> {
    let mut conds = Vec::with_capacity(terms.len());
    let mut binds: Vec<SqlValue> = id_params.to_vec();
    for t in terms {
        conds.push("r.search_text LIKE ? ESCAPE '\\'".to_string());
        binds.push(SqlValue::Text(format!("%{}%", escape_like(t))));
    }
    let sql = format!(
        "SELECT r.source_id, s.name, r.label, r.open_ref, r.row_number, r.fields_json
         FROM records r JOIN sources s ON s.id = r.source_id
         WHERE {in_clause} AND {} LIMIT {limit}",
        conds.join(" AND ")
    );
    let mut stmt = conn.prepare(&sql)?;
    let out = collect_rows(stmt.query(params_from_iter(binds.iter()))?);
    out
}

fn browse(
    conn: &Connection,
    in_clause: &str,
    id_params: &[SqlValue],
    limit: i64,
) -> Result<Vec<Row>> {
    let sql = format!(
        "SELECT r.source_id, s.name, r.label, r.open_ref, r.row_number, r.fields_json
         FROM records r JOIN sources s ON s.id = r.source_id
         WHERE {in_clause} LIMIT {limit}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let out = collect_rows(stmt.query(params_from_iter(id_params.iter()))?);
    out
}

fn like_search(
    conn: &Connection,
    q: &str,
    in_clause: &str,
    id_params: &[SqlValue],
    limit: i64,
) -> Result<Vec<Row>> {
    let pattern = format!("%{}%", escape_like(q));
    let sql = format!(
        "SELECT r.source_id, s.name, r.label, r.open_ref, r.row_number, r.fields_json
         FROM records r JOIN sources s ON s.id = r.source_id
         WHERE {in_clause} AND r.search_text LIKE ? ESCAPE '\\'
         LIMIT {limit}"
    );
    let mut binds: Vec<SqlValue> = id_params.to_vec();
    binds.push(SqlValue::Text(pattern));
    let mut stmt = conn.prepare(&sql)?;
    let out = collect_rows(stmt.query(params_from_iter(binds.iter()))?);
    out
}

fn fts_search(
    conn: &Connection,
    q: &str,
    in_clause: &str,
    id_params: &[SqlValue],
    limit: i64,
) -> Result<Vec<Row>> {
    // trigram では部分一致をフレーズとして与える。内部の " は "" にエスケープ。
    let match_str = format!("\"{}\"", q.replace('"', "\"\""));
    let sql = format!(
        "SELECT sid, sname, label, open_ref, row_number, fields_json FROM (
            SELECT r.source_id AS sid, s.name AS sname, r.label AS label,
                   r.open_ref AS open_ref, r.row_number AS row_number, r.fields_json AS fields_json,
                   ROW_NUMBER() OVER (PARTITION BY r.source_id ORDER BY bm25(records_fts)) AS rn
            FROM records_fts
            JOIN records r ON r.id = records_fts.rowid
            JOIN sources s ON s.id = r.source_id
            WHERE records_fts MATCH ? AND {in_clause}
         )
         WHERE rn <= {PER_SOURCE_CAP}
         LIMIT {limit}"
    );
    let mut binds: Vec<SqlValue> = vec![SqlValue::Text(match_str)];
    binds.extend(id_params.iter().cloned());
    let mut stmt = conn.prepare(&sql)?;
    let out = collect_rows(stmt.query(params_from_iter(binds.iter()))?);
    out
}

fn collect_rows(mut rows: rusqlite::Rows) -> Result<Vec<Row>> {
    let mut out = Vec::new();
    while let Some(r) = rows.next()? {
        let source_id: i64 = r.get(0)?;
        let fields_json: String = r.get(5)?;
        let fields: Map<String, serde_json::Value> = serde_json::from_str(&fields_json)
            .with_context(|| {
                format!("invalid fields_json in cached record for source {source_id}")
            })?;
        out.push(Row {
            source_id,
            source_name: r.get(1)?,
            label: r.get(2)?,
            open_ref: r.get(3)?,
            row_number: r.get(4)?,
            fields,
        });
    }
    Ok(out)
}

// ---- 検索履歴（要件 §9-5）----
pub fn add_search_history(conn: &Connection, query: &str) -> Result<()> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(());
    }
    // 直近と同一クエリなら重複追加しない。
    let last: Option<String> = conn
        .query_row(
            "SELECT query FROM search_history ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .ok();
    if last.as_deref() == Some(q) {
        return Ok(());
    }
    conn.execute("INSERT INTO search_history(query) VALUES (?1)", [q])?;
    conn.execute(
        "DELETE FROM search_history WHERE id NOT IN (SELECT id FROM search_history ORDER BY id DESC LIMIT 200)",
        [],
    )?;
    Ok(())
}

pub fn list_search_history(conn: &Connection, limit: i64) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT query FROM (SELECT query, MAX(id) AS mid FROM search_history GROUP BY query)
         ORDER BY mid DESC LIMIT ?1",
    )?;
    let rows = stmt
        .query_map([limit], |r| r.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

// ---- 検索プリセット/お気に入り（要件 REQ-BM-01）----
pub fn save_search_preset(
    conn: &Connection,
    name: &str,
    query: &str,
    source_ids: &[i64],
) -> Result<i64> {
    let ids_json = serde_json::to_string(source_ids)?;
    conn.execute(
        "INSERT INTO search_presets(name, query, source_ids_json) VALUES(?1, ?2, ?3)",
        params![name, query, ids_json],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_search_presets(conn: &Connection) -> Result<Vec<crate::model::SearchPreset>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, query, source_ids_json, created_at FROM search_presets ORDER BY id DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            let ids_json: String = r.get(3)?;
            let source_ids: Vec<i64> = serde_json::from_str(&ids_json).unwrap_or_default();
            Ok(crate::model::SearchPreset {
                id: r.get(0)?,
                name: r.get(1)?,
                query: r.get(2)?,
                source_ids,
                created_at: r.get(4)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn delete_search_preset(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM search_presets WHERE id = ?1", [id])?;
    Ok(())
}

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rec(name: &str, value: &str) -> Record {
        let mut fields = Map::new();
        fields.insert("name".into(), json!(name));
        fields.insert("value".into(), json!(value));
        Record {
            label: Some(name.into()),
            open_ref: Some(format!("C:\\data\\{name}.txt")),
            row_number: None,
            fields,
            search_text: format!("{name} {value}"),
        }
    }

    fn setup() -> (Connection, i64) {
        let mut conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let sid = insert_source(&conn, "csv", "テスト", "{}").unwrap();
        let recs = vec![
            rec("田中太郎", "営業部 東京"),
            rec("鈴木花子", "開発部 大阪"),
            rec("Tanaka", "sales tokyo"),
        ];
        replace_source_records(&mut conn, sid, &recs).unwrap();
        (conn, sid)
    }

    fn create_legacy_fixture(path: &Path) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE sources (
                id INTEGER PRIMARY KEY, type TEXT NOT NULL, name TEXT NOT NULL,
                config_json TEXT NOT NULL, refresh_interval INTEGER NOT NULL DEFAULT 3600,
                status TEXT, last_success_at TEXT, last_error TEXT
            );
            CREATE TABLE records (
                id INTEGER PRIMARY KEY, source_id INTEGER NOT NULL, label TEXT,
                open_ref TEXT, fields_json TEXT NOT NULL, search_text TEXT NOT NULL
            );
            INSERT INTO sources(id, type, name, config_json, refresh_interval)
              VALUES(7, 'csv', '移行前ソース', '{"path":"C:\\legacy.csv"}', 900);
            INSERT INTO records(source_id, label, open_ref, fields_json, search_text)
              VALUES(7, '移行前レコード', 'file:///C:/legacy.csv', '{"name":"移行前"}', '移行前');
            "#,
        )
        .unwrap();
    }

    fn temporary_database_path(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("everydb-db-test-{name}-{nonce}"));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cache.db");
        (dir, path)
    }

    #[test]
    fn migrates_legacy_fixture_without_losing_sources_or_records() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE sources (
                id INTEGER PRIMARY KEY, type TEXT NOT NULL, name TEXT NOT NULL,
                config_json TEXT NOT NULL, refresh_interval INTEGER NOT NULL DEFAULT 3600,
                status TEXT, last_success_at TEXT, last_error TEXT
            );
            CREATE TABLE records (
                id INTEGER PRIMARY KEY, source_id INTEGER NOT NULL, label TEXT,
                open_ref TEXT, fields_json TEXT NOT NULL, search_text TEXT NOT NULL
            );
            INSERT INTO sources(id, type, name, config_json) VALUES(7, 'csv', '移行前ソース', '{"path":"C:\\legacy.csv"}');
            INSERT INTO records(source_id, label, fields_json, search_text) VALUES(7, '移行前レコード', '{"name":"移行前"}', '移行前');
            "#,
        )
        .unwrap();

        init_schema(&conn).unwrap();

        assert_eq!(
            get_meta(&conn, SCHEMA_VERSION_KEY).unwrap().as_deref(),
            Some("2")
        );
        assert!(table_has_column(&conn, "sources", "sort_order").unwrap());
        assert!(table_has_column(&conn, "records", "row_number").unwrap());
        let (name, config): (String, String) = conn
            .query_row(
                "SELECT name, config_json FROM sources WHERE id=7",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(name, "移行前ソース");
        assert_eq!(config, r#"{"path":"C:\\legacy.csv"}"#);
        assert_eq!(
            conn.query_row::<String, _, _>(
                "SELECT label FROM records WHERE source_id=7",
                [],
                |row| row.get(0)
            )
            .unwrap(),
            "移行前レコード"
        );
    }

    #[test]
    fn file_migration_cleans_up_backup_after_success() {
        let (dir, path) = temporary_database_path("success");
        create_legacy_fixture(&path);

        let conn = open_cached_database(&path).unwrap();
        assert_eq!(
            get_meta(&conn, SCHEMA_VERSION_KEY).unwrap().as_deref(),
            Some("2")
        );
        assert_eq!(list_sources(&conn).unwrap()[0].name, "移行前ソース");
        drop(conn);

        assert!(!path
            .with_file_name("cache.db.before-migration.bak")
            .exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_file_migration_restores_backup() {
        let (dir, path) = temporary_database_path("restore");
        create_legacy_fixture(&path);

        let result =
            initialize_database_with(&path, |_| Err(anyhow::anyhow!("forced migration failure")));
        assert!(result.is_err());

        let restored = Connection::open(&path).unwrap();
        assert_eq!(
            restored
                .query_row::<String, _, _>("SELECT name FROM sources WHERE id=7", [], |row| row
                    .get(0))
                .unwrap(),
            "移行前ソース"
        );
        assert!(!path
            .with_file_name("cache.db.before-migration.bak")
            .exists());
        drop(restored);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn fts_trigram_available_and_matches_japanese() {
        let (conn, sid) = setup();
        // 3 文字以上 → FTS trigram。日本語部分一致。
        let (rows, _) = search(&conn, "田中太", &[sid], DEFAULT_ROW_LIMIT).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label.as_deref(), Some("田中太郎"));
    }

    #[test]
    fn malformed_cached_fields_json_is_reported_instead_of_hidden() {
        let (conn, sid) = setup();
        conn.execute(
            "INSERT INTO records(source_id, label, fields_json, search_text) VALUES(?1, ?2, ?3, ?4)",
            params![sid, "破損レコード", "{not-json", "破損レコード"],
        )
        .unwrap();

        let error = match search(&conn, "", &[sid], DEFAULT_ROW_LIMIT) {
            Ok(_) => panic!("malformed fields_json must fail the search"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains(&format!(
                "invalid fields_json in cached record for source {sid}"
            )),
            "unexpected error: {error}"
        );
    }
    #[test]
    fn like_fallback_for_short_query() {
        let (conn, sid) = setup();
        // 2 文字 → LIKE フォールバック。
        let (rows, _) = search(&conn, "大阪", &[sid], DEFAULT_ROW_LIMIT).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label.as_deref(), Some("鈴木花子"));
    }

    #[test]
    fn empty_query_browses_all() {
        let (conn, sid) = setup();
        let (rows, _) = search(&conn, "", &[sid], DEFAULT_ROW_LIMIT).unwrap();
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn snapshot_replace_removes_old_records() {
        let (mut conn, sid) = setup();
        // 全置換で 1 件だけに → 消えたレコードは FTS からも消える。
        replace_source_records(&mut conn, sid, &[rec("新規", "only one")]).unwrap();
        let (all, _) = search(&conn, "", &[sid], DEFAULT_ROW_LIMIT).unwrap();
        assert_eq!(all.len(), 1);
        let (gone, _) = search(&conn, "田中太", &[sid], DEFAULT_ROW_LIMIT).unwrap();
        assert_eq!(gone.len(), 0);
    }

    #[test]
    fn source_filter_excludes_unselected() {
        let (conn, sid) = setup();
        // 存在しないソース id で絞ると 0 件。
        let (rows, _) = search(&conn, "田中太", &[sid + 999], DEFAULT_ROW_LIMIT).unwrap();
        assert_eq!(rows.len(), 0);
    }

    #[test]
    fn empty_source_selection_returns_nothing() {
        // 全チェックを外した状態は「全ソース対象」ではなく 0 件（要件）。
        let (conn, sid) = setup();
        // 前提: 選択されていれば当然ヒットする。
        assert!(!search(&conn, "田中太", &[sid], DEFAULT_ROW_LIMIT)
            .unwrap()
            .0
            .is_empty());
        assert!(!search(&conn, "", &[sid], DEFAULT_ROW_LIMIT)
            .unwrap()
            .0
            .is_empty());
        // 選択が空なら検索もブラウズも 0 件、かつ打ち切り扱いにもしない。
        let (rows, truncated) = search(&conn, "田中太", &[], DEFAULT_ROW_LIMIT).unwrap();
        assert!(rows.is_empty());
        assert!(!truncated);
        assert!(search(&conn, "", &[], DEFAULT_ROW_LIMIT)
            .unwrap()
            .0
            .is_empty());
    }

    #[test]
    fn delete_source_removes_records_and_fts() {
        let (conn, sid) = setup();
        delete_source(&conn, sid).unwrap();
        // ブラウズも FTS 検索も 0 件（FTS に取り残しが無い）。
        // 空の source_ids は「0 件」の意味になったため、削除したソースを明示的に指定して
        // 取り残しの有無を確かめる。
        let (browse, _) = search(&conn, "", &[sid], DEFAULT_ROW_LIMIT).unwrap();
        assert_eq!(browse.len(), 0);
        let (fts, _) = search(&conn, "田中太", &[sid], DEFAULT_ROW_LIMIT).unwrap();
        assert_eq!(fts.len(), 0);
        // sources 自体も消えている。
        assert_eq!(list_sources(&conn).unwrap().len(), 0);
    }

    #[test]
    fn set_interval_updates_and_clamps() {
        let (conn, sid) = setup();
        set_refresh_interval(&conn, sid, 1800).unwrap();
        assert_eq!(list_sources(&conn).unwrap()[0].refresh_interval, 1800);
        // 下限 10 秒でクランプ。
        set_refresh_interval(&conn, sid, 1).unwrap();
        assert_eq!(list_sources(&conn).unwrap()[0].refresh_interval, 10);
    }

    #[test]
    fn source_parent_groups_by_path_site_and_service() {
        // ローカル系 → path がそのまま親キー（クォート付きも正規化）。
        assert_eq!(
            source_parent("excel_table", r#"{"path":"\"C:\\a\\b.xlsx\"","sheet":"S"}"#),
            "C:\\a\\b.xlsx"
        );
        assert_eq!(
            source_parent("sqlite", r#"{"path":"C:\\db.sqlite","table":"t"}"#),
            "C:\\db.sqlite"
        );
        // SharePoint → site_url が親キー。
        assert_eq!(
            source_parent(
                "graph_sharepoint_list",
                r#"{"site_url":"https://x/sites/y","list_name":"L"}"#
            ),
            "https://x/sites/y"
        );
        // Notion / Graph 非 SharePoint 系 → サービス単位でまとめる。
        assert_eq!(
            source_parent("notion", r#"{"token":"t","database_id":"d"}"#),
            "Notion"
        );
        assert_eq!(
            source_parent("graph_entra_users", r#"{"tenant_id":"t"}"#),
            "Microsoft 365"
        );
        // PostgreSQL/MySQL → host+database が親キー。Box → 保存済みトークン(cred:<id>)単位。
        assert_eq!(
            source_parent(
                "postgres",
                r#"{"host":"db.example.com","database":"app","table":"t"}"#
            ),
            "postgres://db.example.com/app"
        );
        assert_eq!(
            source_parent(
                "mysql",
                r#"{"host":"localhost","database":"shop","table":"t"}"#
            ),
            "mysql://localhost/shop"
        );
        assert_eq!(
            source_parent("box_files", r#"{"access_token":"cred:7","folder_id":"0"}"#),
            "Box (トークン#7)"
        );
        // Airtable → Base 単位（名称優先・次点 ID）でまとめる。
        assert_eq!(
            source_parent("airtable", r#"{"base_id":"app123","table":"tbl1"}"#),
            "Airtable (app123)"
        );
        assert_eq!(
            source_parent(
                "airtable",
                r#"{"base_id":"app123","base_name":"Projects","table":"tbl1"}"#
            ),
            "Airtable: Projects"
        );
        // 親を持たないソースは空文字（単独ノード扱い）。
        assert_eq!(source_parent("ime_msime", r#"{}"#), "");
    }

    #[test]
    fn cached_field_names_reads_from_stored_records() {
        let (conn, sid) = setup();
        let names = cached_field_names(&conn, sid).unwrap();
        assert_eq!(names, vec!["name".to_string(), "value".to_string()]);
        // レコードの無いソースは空。
        assert!(cached_field_names(&conn, sid + 999).unwrap().is_empty());
    }

    #[test]
    fn parent_label_override_roundtrip() {
        let (conn, _sid) = setup();
        assert!(list_parent_labels(&conn).unwrap().is_empty());
        set_parent_label(&conn, "C:\\a\\b.xlsx", "経理システム").unwrap();
        set_parent_label(&conn, "https://x/sites/y", "共有サイト").unwrap();
        let mut labels = list_parent_labels(&conn).unwrap();
        labels.sort();
        assert_eq!(
            labels,
            vec![
                ("C:\\a\\b.xlsx".to_string(), "経理システム".to_string()),
                ("https://x/sites/y".to_string(), "共有サイト".to_string()),
            ]
        );
        // 空文字での再設定は削除として扱われる。
        set_parent_label(&conn, "C:\\a\\b.xlsx", "").unwrap();
        let labels = list_parent_labels(&conn).unwrap();
        assert_eq!(
            labels,
            vec![("https://x/sites/y".to_string(), "共有サイト".to_string())]
        );
    }

    #[test]
    fn search_preset_crud_roundtrip() {
        let (conn, sid) = setup();
        let id = save_search_preset(&conn, "お気に入り1", "田中", &[sid]).unwrap();
        let presets = list_search_presets(&conn).unwrap();
        assert_eq!(presets.len(), 1);
        assert_eq!(presets[0].id, id);
        assert_eq!(presets[0].name, "お気に入り1");
        assert_eq!(presets[0].query, "田中");
        assert_eq!(presets[0].source_ids, vec![sid]);

        delete_search_preset(&conn, id).unwrap();
        assert!(list_search_presets(&conn).unwrap().is_empty());
    }
}

use anyhow::Result;
use rusqlite::{params, params_from_iter, types::Value as SqlValue, Connection};
use serde_json::Map;

use crate::model::{Record, Row, SourceInfo};

const PER_SOURCE_CAP: i64 = 200;
/// 行数上限の既定値（UI から変更可能）。
pub const DEFAULT_ROW_LIMIT: i64 = 10000;

/// スキーマ初期化（要件 §5-2）。検索専用の records_fts は external content + trigram。
pub fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;

        CREATE TABLE IF NOT EXISTS sources (
            id               INTEGER PRIMARY KEY,
            type             TEXT NOT NULL,
            name             TEXT NOT NULL,
            config_json      TEXT NOT NULL,
            refresh_interval INTEGER NOT NULL DEFAULT 3600,
            status           TEXT,
            last_success_at  TEXT,
            last_error       TEXT,
            sort_order       INTEGER NOT NULL DEFAULT 0
        );

        -- アプリ全体の単一値設定（マスターパスワードハッシュ等）。要件 §9-4。
        CREATE TABLE IF NOT EXISTS app_meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS search_history (
            id         INTEGER PRIMARY KEY,
            query      TEXT NOT NULL,
            used_at    TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        );

        -- 検索プリセット/お気に入り（要件 REQ-BM-01）。
        CREATE TABLE IF NOT EXISTS search_presets (
            id              INTEGER PRIMARY KEY,
            name            TEXT NOT NULL,
            query           TEXT NOT NULL DEFAULT '',
            source_ids_json TEXT NOT NULL DEFAULT '[]',
            created_at      TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        );

        CREATE TABLE IF NOT EXISTS records (
            id          INTEGER PRIMARY KEY,
            source_id   INTEGER NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
            label       TEXT,
            open_ref    TEXT,
            row_number  INTEGER,
            fields_json TEXT NOT NULL,
            search_text TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_records_source ON records(source_id);

        CREATE VIRTUAL TABLE IF NOT EXISTS records_fts USING fts5(
            search_text,
            content='records',
            content_rowid='id',
            tokenize='trigram'
        );

        CREATE TRIGGER IF NOT EXISTS records_ai AFTER INSERT ON records BEGIN
            INSERT INTO records_fts(rowid, search_text) VALUES (new.id, new.search_text);
        END;
        CREATE TRIGGER IF NOT EXISTS records_ad AFTER DELETE ON records BEGIN
            INSERT INTO records_fts(records_fts, rowid, search_text)
              VALUES ('delete', old.id, old.search_text);
        END;
        CREATE TRIGGER IF NOT EXISTS records_au AFTER UPDATE ON records BEGIN
            INSERT INTO records_fts(records_fts, rowid, search_text)
              VALUES ('delete', old.id, old.search_text);
            INSERT INTO records_fts(rowid, search_text) VALUES (new.id, new.search_text);
        END;
        "#,
    )?;
    migrate_add_column(conn, "sources", "sort_order", "INTEGER NOT NULL DEFAULT 0")?;
    migrate_add_column(conn, "records", "row_number", "INTEGER")?;
    Ok(())
}

/// 既存 DB に後付けで列を追加する簡易マイグレーション（列が無い場合のみ ALTER する）。
fn migrate_add_column(conn: &Connection, table: &str, column: &str, decl: &str) -> Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let exists = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .any(|c| c == column);
    if !exists {
        conn.execute(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"), [])?;
    }
    Ok(())
}

pub fn insert_source(conn: &Connection, type_: &str, name: &str, config_json: &str) -> Result<i64> {
    let next_order: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order), 0) + 1 FROM sources", [], |r| r.get(0))?;
    conn.execute(
        "INSERT INTO sources(type, name, config_json, sort_order) VALUES(?1, ?2, ?3, ?4)",
        params![type_, name, config_json, next_order],
    )?;
    Ok(conn.last_insert_rowid())
}

/// 既存ソースの接続情報・フィールド役割・更新間隔を更新する（要件 §9-1: 編集）。
pub fn update_source(conn: &Connection, id: i64, name: &str, config_json: &str) -> Result<()> {
    conn.execute(
        "UPDATE sources SET name = ?2, config_json = ?3 WHERE id = ?1",
        params![id, name, config_json],
    )?;
    Ok(())
}

/// ソースを複製する（要件 §9-1）。config_json はそのままコピー（秘密は暗号化済みのため安全）。
pub fn duplicate_source(conn: &Connection, id: i64, new_name: &str) -> Result<i64> {
    let (type_, config_json) = get_source_config(conn, id)?;
    let refresh_interval: i64 = conn.query_row("SELECT refresh_interval FROM sources WHERE id=?1", [id], |r| r.get(0))?;
    let next_order: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order), 0) + 1 FROM sources", [], |r| r.get(0))?;
    conn.execute(
        "INSERT INTO sources(type, name, config_json, refresh_interval, sort_order) VALUES(?1, ?2, ?3, ?4, ?5)",
        params![type_, new_name, config_json, refresh_interval, next_order],
    )?;
    Ok(conn.last_insert_rowid())
}

/// サイドバーでのドラッグ並び替えを永続化する（要件 §9-1）。
pub fn reorder_sources(conn: &Connection, ordered_ids: &[i64]) -> Result<()> {
    let tx_ok = (|| -> Result<()> {
        for (i, id) in ordered_ids.iter().enumerate() {
            conn.execute("UPDATE sources SET sort_order = ?2 WHERE id = ?1", params![id, i as i64])?;
        }
        Ok(())
    })();
    tx_ok
}

// ---- app_meta（マスターパスワード等の単一値設定）----
pub fn get_meta(conn: &Connection, key: &str) -> Result<Option<String>> {
    let v = conn
        .query_row("SELECT value FROM app_meta WHERE key = ?1", [key], |r| r.get::<_, String>(0))
        .ok();
    Ok(v)
}

pub fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO app_meta(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn delete_meta(conn: &Connection, key: &str) -> Result<()> {
    conn.execute("DELETE FROM app_meta WHERE key = ?1", [key])?;
    Ok(())
}

// ---- 親（ファイル/サーバー）ノードの表示名オーバーライド ----
// parent キー（path / site_url / "Notion" 等）ごとに任意の分かりやすい表示名を付けられるようにする。
// parent は DB 行を持たない仮想グルーピングのため、app_meta に "parent_label:<parentKey>" として保持する。
const PARENT_LABEL_PREFIX: &str = "parent_label:";

pub fn set_parent_label(conn: &Connection, parent_key: &str, label: &str) -> Result<()> {
    let key = format!("{PARENT_LABEL_PREFIX}{parent_key}");
    if label.trim().is_empty() {
        delete_meta(conn, &key)
    } else {
        set_meta(conn, &key, label.trim())
    }
}

pub fn list_parent_labels(conn: &Connection) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT key, value FROM app_meta WHERE key LIKE 'parent_label:%'")?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .map(|(k, v)| (k.trim_start_matches(PARENT_LABEL_PREFIX).to_string(), v))
        .collect())
}

/// データソースに接続できない場合のフォールバック: 既にキャッシュ済みのレコードから
/// フィールド名一覧を復元する（出現順・重複無し）。ライブ接続不能でも表示/検索設定を編集できるように。
/// 注意: ここで返る名前は表示名エイリアス適用後のもの（キャッシュは Record.fields をそのまま保存しているため）。
pub fn cached_field_names(conn: &Connection, source_id: i64) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT fields_json FROM records WHERE source_id = ?1 LIMIT 200")?;
    let jsons: Vec<String> = stmt
        .query_map([source_id], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut seen = std::collections::HashSet::new();
    let mut names = Vec::new();
    for j in jsons {
        let v: serde_json::Value = match serde_json::from_str(&j) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(obj) = v.as_object() {
            for k in obj.keys() {
                if seen.insert(k.clone()) {
                    names.push(k.clone());
                }
            }
        }
    }
    Ok(names)
}

pub fn get_source_config(conn: &Connection, id: i64) -> Result<(String, String)> {
    let row = conn.query_row(
        "SELECT type, config_json FROM sources WHERE id = ?1",
        [id],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
    )?;
    Ok(row)
}

/// 閲覧・編集ダイアログ用に、ソースの全属性をまとめて取得する（要件 §9-1）。
pub fn get_source_full(conn: &Connection, id: i64) -> Result<(String, String, i64, String)> {
    conn.query_row(
        "SELECT type, name, refresh_interval, config_json FROM sources WHERE id = ?1",
        [id],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, String>(3)?)),
    )
    .map_err(Into::into)
}

pub fn list_sources(conn: &Connection) -> Result<Vec<SourceInfo>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.type, s.name, s.refresh_interval, s.status, s.last_success_at, s.last_error,
                (SELECT COUNT(*) FROM records r WHERE r.source_id = s.id), s.config_json
         FROM sources s ORDER BY s.sort_order, s.id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            let type_: String = r.get(1)?;
            let config_json: String = r.get(8)?;
            Ok(SourceInfo {
                id: r.get(0)?,
                parent: source_parent(&type_, &config_json),
                type_,
                name: r.get(2)?,
                refresh_interval: r.get(3)?,
                status: r.get(4)?,
                last_success_at: r.get(5)?,
                last_error: r.get(6)?,
                record_count: r.get(7)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// ツリー表示の親キーを config から算出する（path / site_url / database_id 等。秘密は含めない）。
fn source_parent(type_: &str, config_json: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(config_json).unwrap_or_default();
    if let Some(p) = v.get("path").and_then(|x| x.as_str()) {
        return crate::connectors::strip_path_quotes(p);
    }
    if let Some(u) = v.get("site_url").and_then(|x| x.as_str()) {
        return u.to_string();
    }
    if type_ == "notion" {
        // データベース単位ではなく「Notion」というサーバー単位でまとめる
        // （Microsoft 365 と同じ扱い。個々の DB は子のテーブル相当）。
        return "Notion".to_string();
    }
    if type_ == "airtable" {
        // Airtable は Base 単位をツリーの親とし、個々の Table / View を子のテーブル相当としてまとめる。
        let name = v.get("base_name").and_then(|x| x.as_str()).unwrap_or("");
        let id = v.get("base_id").and_then(|x| x.as_str()).unwrap_or("");
        if !name.is_empty() {
            return format!("Airtable: {name}");
        } else if !id.is_empty() {
            return format!("Airtable ({id})");
        }
        return "Airtable".to_string();
    }
    if type_ == "pokeapi" {
        return "PokéAPI".to_string();
    }
    if type_.starts_with("graph") {
        return "Microsoft 365".to_string();
    }
    if type_ == "postgres" || type_ == "mysql" {
        let host = v.get("host").and_then(|x| x.as_str()).unwrap_or("");
        let database = v.get("database").and_then(|x| x.as_str()).unwrap_or("");
        if !host.is_empty() && !database.is_empty() {
            return format!("{type_}://{host}/{database}");
        }
    }
    if type_ == "box_files" {
        if let Some(eid) = v.get("enterprise_id").and_then(|x| x.as_str()) {
            if !eid.is_empty() {
                return format!("Box ({eid})");
            }
        }
    }
    // 親が無いソースは自分自身を親扱いにする（単独ノード）。
    String::new()
}

pub fn delete_source(conn: &Connection, id: i64) -> Result<()> {
    // records を明示削除して records_ad トリガーで FTS を確実に同期させる
    // (FK カスケードでのトリガー発火は recursive_triggers 設定依存で不確実なため)。
    conn.execute("DELETE FROM records WHERE source_id = ?1", [id])?;
    conn.execute("DELETE FROM sources WHERE id = ?1", [id])?;
    Ok(())
}

pub fn set_refresh_interval(conn: &Connection, id: i64, seconds: i64) -> Result<()> {
    let secs = seconds.max(10); // 最低 10 秒
    conn.execute(
        "UPDATE sources SET refresh_interval = ?2 WHERE id = ?1",
        params![id, secs],
    )?;
    Ok(())
}

pub fn set_status(conn: &Connection, id: i64, status: &str, error: Option<&str>) -> Result<()> {
    if status == "ok" {
        conn.execute(
            "UPDATE sources SET status='ok', last_success_at=datetime('now','localtime'), last_error=NULL WHERE id=?1",
            [id],
        )?;
    } else {
        conn.execute(
            "UPDATE sources SET status=?2, last_error=?3 WHERE id=?1",
            params![id, status, error],
        )?;
    }
    Ok(())
}

/// スナップショット全置換（要件 §4-1）。トランザクション内で全削除 → 再挿入。
pub fn replace_source_records(conn: &mut Connection, source_id: i64, records: &[Record]) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM records WHERE source_id = ?1", [source_id])?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO records(source_id, label, open_ref, row_number, fields_json, search_text)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for r in records {
            let fields_json = serde_json::to_string(&r.fields)?;
            stmt.execute(params![source_id, r.label, r.open_ref, r.row_number, fields_json, r.search_text])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// 検索（要件 §5-2 / §6-2 / §9-5）。
/// 空クエリはブラウズ、単一語は 1〜2 文字 LIKE / 3 文字以上 FTS、
/// **半角スペース区切りの複数語は AND 検索**（全語を LIKE で AND 結合、要件 §9-5）。
pub fn search(conn: &Connection, query: &str, source_ids: &[i64], limit: i64) -> Result<(Vec<Row>, bool)> {
    let limit = limit.clamp(1, 1_000_000); // 行数上限（要件: ユーザー設定可能）
    let terms: Vec<&str> = query.split(' ').map(|t| t.trim()).filter(|t| !t.is_empty()).collect();

    // IN 句の構築（空なら全ソース = 制約なし）
    let (in_clause, id_params): (String, Vec<SqlValue>) = if source_ids.is_empty() {
        ("1=1".to_string(), vec![])
    } else {
        let ph = vec!["?"; source_ids.len()].join(",");
        (
            format!("r.source_id IN ({ph})"),
            source_ids.iter().map(|&i| SqlValue::Integer(i)).collect(),
        )
    };

    let rows = match terms.as_slice() {
        [] => browse(conn, &in_clause, &id_params, limit)?,
        [single] if single.chars().count() < 3 => like_search(conn, single, &in_clause, &id_params, limit)?,
        [single] => fts_search(conn, single, &in_clause, &id_params, limit)?,
        multiple => multi_term_and_search(conn, multiple, &in_clause, &id_params, limit)?,
    };

    let truncated = rows.len() as i64 >= limit;
    Ok((rows, truncated))
}

/// 複数語 AND 検索。全語を `search_text LIKE` の AND で結合する
/// （語ごとに FTS/LIKE を混在させると MATCH の複数回結合が煩雑になるため、
/// 複数語ケースは LIKE の AND に統一する。対象件数は TOTAL_CAP で頭打ちのため十分高速）。
fn multi_term_and_search(conn: &Connection, terms: &[&str], in_clause: &str, id_params: &[SqlValue], limit: i64) -> Result<Vec<Row>> {
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

fn browse(conn: &Connection, in_clause: &str, id_params: &[SqlValue], limit: i64) -> Result<Vec<Row>> {
    let sql = format!(
        "SELECT r.source_id, s.name, r.label, r.open_ref, r.row_number, r.fields_json
         FROM records r JOIN sources s ON s.id = r.source_id
         WHERE {in_clause} LIMIT {limit}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let out = collect_rows(stmt.query(params_from_iter(id_params.iter()))?);
    out
}

fn like_search(conn: &Connection, q: &str, in_clause: &str, id_params: &[SqlValue], limit: i64) -> Result<Vec<Row>> {
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

fn fts_search(conn: &Connection, q: &str, in_clause: &str, id_params: &[SqlValue], limit: i64) -> Result<Vec<Row>> {
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
        let fields_json: String = r.get(5)?;
        let fields: Map<String, serde_json::Value> =
            serde_json::from_str(&fields_json).unwrap_or_default();
        out.push(Row {
            source_id: r.get(0)?,
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
        .query_row("SELECT query FROM search_history ORDER BY id DESC LIMIT 1", [], |r| r.get(0))
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
    let rows = stmt.query_map([limit], |r| r.get::<_, String>(0))?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

// ---- 検索プリセット/お気に入り（要件 REQ-BM-01）----
pub fn save_search_preset(conn: &Connection, name: &str, query: &str, source_ids: &[i64]) -> Result<i64> {
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
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
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

    #[test]
    fn fts_trigram_available_and_matches_japanese() {
        let (conn, sid) = setup();
        // 3 文字以上 → FTS trigram。日本語部分一致。
        let (rows, _) = search(&conn, "田中太", &[sid], DEFAULT_ROW_LIMIT).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label.as_deref(), Some("田中太郎"));
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
    fn delete_source_removes_records_and_fts() {
        let (conn, sid) = setup();
        delete_source(&conn, sid).unwrap();
        // ブラウズも FTS 検索も 0 件（FTS に取り残しが無い）。
        let (browse, _) = search(&conn, "", &[], DEFAULT_ROW_LIMIT).unwrap();
        assert_eq!(browse.len(), 0);
        let (fts, _) = search(&conn, "田中太", &[], DEFAULT_ROW_LIMIT).unwrap();
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
        assert_eq!(source_parent("excel_table", r#"{"path":"\"C:\\a\\b.xlsx\"","sheet":"S"}"#), "C:\\a\\b.xlsx");
        assert_eq!(source_parent("sqlite", r#"{"path":"C:\\db.sqlite","table":"t"}"#), "C:\\db.sqlite");
        // SharePoint → site_url が親キー。
        assert_eq!(
            source_parent("graph_sharepoint_list", r#"{"site_url":"https://x/sites/y","list_name":"L"}"#),
            "https://x/sites/y"
        );
        // Notion / Graph 非 SharePoint 系 → サービス単位でまとめる。
        assert_eq!(source_parent("notion", r#"{"token":"t","database_id":"d"}"#), "Notion");
        assert_eq!(source_parent("graph_entra_users", r#"{"tenant_id":"t"}"#), "Microsoft 365");
        // PostgreSQL/MySQL → host+database が親キー。Box → enterprise_id が親キー。
        assert_eq!(
            source_parent("postgres", r#"{"host":"db.example.com","database":"app","table":"t"}"#),
            "postgres://db.example.com/app"
        );
        assert_eq!(
            source_parent("mysql", r#"{"host":"localhost","database":"shop","table":"t"}"#),
            "mysql://localhost/shop"
        );
        assert_eq!(
            source_parent("box_files", r#"{"client_id":"c","enterprise_id":"12345"}"#),
            "Box (12345)"
        );
        // Airtable → Base 単位（名称優先・次点 ID）でまとめる。
        assert_eq!(
            source_parent("airtable", r#"{"base_id":"app123","table":"tbl1"}"#),
            "Airtable (app123)"
        );
        assert_eq!(
            source_parent("airtable", r#"{"base_id":"app123","base_name":"Projects","table":"tbl1"}"#),
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
        assert_eq!(labels, vec![("https://x/sites/y".to_string(), "共有サイト".to_string())]);
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

use anyhow::{Context, Result};
use rusqlite::{params, params_from_iter, types::Value as SqlValue, Connection};
use serde_json::Map;

use crate::model::{Row, SearchPreset};

const PER_SOURCE_CAP: i64 = 200;
/// 検索（要件 §5-2 / §6-2 / §9-5）。
/// 空クエリはブラウズ、単一語は 1〜2 文字 LIKE / 3 文字以上 FTS、
/// **半角スペース区切りの複数語は AND 検索**（長語は FTS、短語は LIKE、要件 §9-5）。
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

/// 複数語 AND 検索。3文字以上は FTS5 trigram、1〜2文字は LIKE を使う。
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
        if t.chars().count() >= 3 {
            conds.push(
                "r.id IN (SELECT rowid FROM records_fts WHERE records_fts MATCH ?)".to_string(),
            );
            binds.push(SqlValue::Text(fts_phrase(t)));
        } else {
            conds.push("r.search_text LIKE ? ESCAPE '\\'".to_string());
            binds.push(SqlValue::Text(format!("%{}%", escape_like(t))));
        }
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
    let match_str = fts_phrase(q);
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

pub fn list_search_presets(conn: &Connection) -> Result<Vec<SearchPreset>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, query, source_ids_json, created_at FROM search_presets ORDER BY id DESC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            let ids_json: String = r.get(3)?;
            let source_ids: Vec<i64> = serde_json::from_str(&ids_json).unwrap_or_default();
            Ok(SearchPreset {
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

/// trigram の部分一致へ安全なフレーズとして渡す。入力を FTS 構文にしない。
fn fts_phrase(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

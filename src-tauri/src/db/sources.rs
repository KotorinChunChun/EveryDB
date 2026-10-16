use anyhow::Result;
use rusqlite::{params, Connection};

use crate::model::SourceInfo;
pub fn insert_source(conn: &Connection, type_: &str, name: &str, config_json: &str) -> Result<i64> {
    let next_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM sources",
        [],
        |r| r.get(0),
    )?;
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
    let refresh_interval: i64 = conn.query_row(
        "SELECT refresh_interval FROM sources WHERE id=?1",
        [id],
        |r| r.get(0),
    )?;
    let next_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM sources",
        [],
        |r| r.get(0),
    )?;
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
            conn.execute(
                "UPDATE sources SET sort_order = ?2 WHERE id = ?1",
                params![id, i as i64],
            )?;
        }
        Ok(())
    })();
    tx_ok
}

// ---- app_meta（マスターパスワード等の単一値設定）----
pub fn get_meta(conn: &Connection, key: &str) -> Result<Option<String>> {
    let v = conn
        .query_row("SELECT value FROM app_meta WHERE key = ?1", [key], |r| {
            r.get::<_, String>(0)
        })
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
    let mut stmt =
        conn.prepare("SELECT key, value FROM app_meta WHERE key LIKE 'parent_label:%'")?;
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
    let mut stmt =
        conn.prepare("SELECT fields_json FROM records WHERE source_id = ?1 LIMIT 200")?;
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
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
            ))
        },
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
pub(super) fn source_parent(type_: &str, config_json: &str) -> String {
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
        // box は Enterprise ID ではなく保存済みトークン（cred:<id>）単位でまとめる
        // （§7-17: 個人プランは OAuth 2.0 のみ対応。Enterprise ID の概念自体が無い）。
        if let Some(tok) = v.get("access_token").and_then(|x| x.as_str()) {
            if let Some(id) = tok.strip_prefix("cred:") {
                return format!("Box (トークン#{id})");
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

//! 保存済みトークンの管理・再利用（要件: トークン管理画面）。
//!
//! ソースの config_json は、秘密フィールドとして実値を直接持つ代わりに `cred:<id>` という
//! 参照を持てる（`secret::CRED_MARKER`）。実際の値は `credentials` テーブルに一度だけ保持し、
//! 「置換」（`update_credential`）すれば参照している全ソースへ自動的に反映される（共有参照方式）。
//! 使用中のトークンは `delete_credential` でブロックされる。

use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::secret;

/// 対応スコープ（`sharedKeysFor` と同じ発想でフィールドをひとまとめにする単位）。
pub const SCOPES: &[&str] = &["notion", "airtable", "graph", "box", "postgres", "mysql"];

/// このスコープの中で、実際に `cred:<id>` の生きた参照になる唯一のフィールド。
/// それ以外のフィールド（tenant_id 等）は選択時に平文コピーされるだけで、置換には追随しない。
fn secret_key_of_scope(scope: &str) -> Option<&'static str> {
    match scope {
        "notion" | "airtable" => Some("token"),
        "graph" | "box" => Some("client_secret"),
        "postgres" | "mysql" => Some("password"),
        _ => None,
    }
}

/// ソース種別からスコープを判定する（`secret::secret_keys` と対になる分類）。
fn scope_of_type(type_: &str) -> Option<&'static str> {
    match type_ {
        "notion" => Some("notion"),
        "airtable" => Some("airtable"),
        "postgres" => Some("postgres"),
        "mysql" => Some("mysql"),
        "box_files" => Some("box"),
        t if t.starts_with("graph") => Some("graph"),
        _ => None,
    }
}

fn validate_scope(scope: &str) -> Result<()> {
    if SCOPES.contains(&scope) {
        Ok(())
    } else {
        Err(anyhow!("未対応のスコープです: {scope}"))
    }
}

#[derive(Serialize)]
pub struct CredentialUsage {
    pub source_id: i64,
    pub source_name: String,
}

#[derive(Serialize)]
pub struct CredentialSummary {
    pub id: i64,
    pub scope: String,
    pub label: String,
    pub created_at: String,
    pub used_by: Vec<CredentialUsage>,
}

/// 指定スコープの保存済みトークン一覧（使用中ソースの情報つき）。
pub fn list_credentials(conn: &Connection, scope: &str) -> Result<Vec<CredentialSummary>> {
    validate_scope(scope)?;
    let mut stmt = conn.prepare(
        "SELECT id, scope, label, created_at FROM credentials WHERE scope = ?1 ORDER BY created_at DESC",
    )?;
    let rows = stmt
        .query_map([scope], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut out = Vec::new();
    for (id, scope, label, created_at) in rows {
        let used_by = find_usage(conn, id)?
            .into_iter()
            .map(|(source_id, source_name)| CredentialUsage { source_id, source_name })
            .collect();
        out.push(CredentialSummary { id, scope, label, created_at, used_by });
    }
    Ok(out)
}

/// 新規保存。スコープの秘密フィールドだけ DPAPI 暗号化する。
pub fn add_credential(conn: &Connection, scope: &str, label: &str, fields_json: &str) -> Result<i64> {
    validate_scope(scope)?;
    let secret_key = secret_key_of_scope(scope);
    let protected = match secret_key {
        Some(k) => secret::protect_json_keys(&[k], fields_json)?,
        None => fields_json.to_string(),
    };
    conn.execute(
        "INSERT INTO credentials(scope, label, fields_json) VALUES(?1, ?2, ?3)",
        params![scope, label, protected],
    )?;
    Ok(conn.last_insert_rowid())
}

/// 「置換」: 値を書き換える。プレースホルダーのままの秘密フィールドは旧暗号化値を維持する
/// （ソース編集と同じ意味論）。参照元の全ソースは次回利用時に自動的に新しい値を使う。
pub fn update_credential(conn: &Connection, id: i64, label: &str, fields_json: &str) -> Result<()> {
    let (scope, old_fields_json): (String, String) = conn.query_row(
        "SELECT scope, fields_json FROM credentials WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let secret_key = secret_key_of_scope(&scope);
    let merged = match secret_key {
        Some(k) => secret::merge_preserving_placeholder_keys(&[k], &old_fields_json, fields_json)?,
        None => fields_json.to_string(),
    };
    let protected = match secret_key {
        Some(k) => secret::protect_json_keys(&[k], &merged)?,
        None => merged,
    };
    conn.execute(
        "UPDATE credentials SET label = ?2, fields_json = ?3 WHERE id = ?1",
        params![id, label, protected],
    )?;
    Ok(())
}

/// 使用中ソースがあれば拒否する（要件: 削除をブロックし使用中ソース一覧を表示）。
pub fn delete_credential(conn: &Connection, id: i64) -> Result<(), String> {
    let usage = find_usage(conn, id).map_err(|e| e.to_string())?;
    if !usage.is_empty() {
        let names: Vec<String> = usage.iter().map(|(_, name)| name.clone()).collect();
        return Err(format!(
            "{}件のソースで使用中のため削除できません: {}\n先に別のトークンへ付け替えるか、それらのソースを削除してください",
            usage.len(),
            names.join("、")
        ));
    }
    conn.execute("DELETE FROM credentials WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

/// 指定の保存済みトークンを参照している (source_id, source_name) の一覧。
pub fn find_usage(conn: &Connection, id: i64) -> Result<Vec<(i64, String)>> {
    let marker = format!("{}{id}", secret::CRED_MARKER);
    let mut stmt = conn.prepare("SELECT id, type, name, config_json FROM sources")?;
    let rows = stmt
        .query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut out = Vec::new();
    for (source_id, type_, name, config_json) in rows {
        let Some(scope) = scope_of_type(&type_) else { continue };
        let Some(key) = secret_key_of_scope(scope) else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&config_json) else { continue };
        if v.get(key).and_then(|x| x.as_str()) == Some(marker.as_str()) {
            out.push((source_id, name));
        }
    }
    Ok(out)
}

/// ピッカー用: ラベルと非秘密フィールドの平文値だけを返す。秘密フィールドの実値は
/// フロントへ一切渡さない（フロントは config に `cred:<id>` を埋めるだけで済む）。
pub fn public_fields(conn: &Connection, id: i64) -> Result<(String, Map<String, Value>)> {
    let (scope, label, fields_json): (String, String, String) = conn.query_row(
        "SELECT scope, label, fields_json FROM credentials WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let secret_key = secret_key_of_scope(&scope);
    let v: Value = serde_json::from_str(&fields_json)?;
    let mut out = Map::new();
    if let Some(obj) = v.as_object() {
        for (k, val) in obj {
            if Some(k.as_str()) == secret_key {
                continue; // 秘密フィールドは除外
            }
            out.insert(k.clone(), val.clone());
        }
    }
    Ok((label, out))
}

/// 指定した保存済みトークンの「秘密フィールド」だけを復号して返す。
///
/// まだソースとして保存していない新規追加フロー（Airtable の Base 一覧、Notion の
/// データベース一覧等をライブ取得するボタン）で、選択済みトークンを使って API 呼び出しを行うために
/// 使う。通常のピッカー（`public_fields`）とは異なり秘密フィールドの実値を返すが、露出の水準は
/// 「トークンを直接入力していた従来の挙動」と同じで、ユーザーが明示的にブラウズ操作を
/// トリガーしたときだけ・その場限りの利用に留めている（保存はされない）。
pub fn resolve_secret_value(conn: &Connection, id: i64) -> Result<String> {
    let (scope, fields_json): (String, String) = conn.query_row(
        "SELECT scope, fields_json FROM credentials WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let key = secret_key_of_scope(&scope).ok_or_else(|| anyhow!("秘密フィールドの無いスコープです: {scope}"))?;
    let decrypted = secret::unprotect_config(&fields_json)?;
    let v: Value = serde_json::from_str(&decrypted)?;
    Ok(v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string())
}

/// 指定 config_json のうち、`cred:<id>` 参照になっているフィールドのキー→トークンのラベル。
/// マスターパスワードの解錠状態に関わらず呼べる（ラベルは秘密ではない。§7-3）。
/// 呼び出し側は主に 2 用途で使う: (1) UI に「🔗 使用中: <ラベル>」を表示する、
/// (2) 解錠時でも `cred:` 参照フィールドは実値を出さずマスクし直す（下記 `remask_cred_refs`）。
pub fn cred_ref_labels(conn: &Connection, config_json: &str) -> Result<Map<String, Value>> {
    let v: Value = serde_json::from_str(config_json)?;
    let mut out = Map::new();
    let Some(obj) = v.as_object() else { return Ok(out) };
    for (k, val) in obj {
        let Some(s) = val.as_str() else { continue };
        let Some(id_str) = s.strip_prefix(secret::CRED_MARKER) else { continue };
        let Ok(id) = id_str.parse::<i64>() else { continue };
        if let Ok(label) = conn.query_row("SELECT label FROM credentials WHERE id = ?1", [id], |r| r.get::<_, String>(0)) {
            // id も一緒に返す（フロントの「トークンをコピー」ボタンが、このセッションで
            // 選び直したかどうかに関わらず credential_secret_value を呼べるようにするため）。
            out.insert(k.clone(), serde_json::json!({ "id": id, "label": label }));
        }
    }
    Ok(out)
}

/// 解錠時であっても、`cred:<id>` 参照だったフィールドは実値を見せず常にマスクし直す。
/// 参照フィールドの「実体」はソースではなく `credentials` 側にあるため、ソース編集画面の
/// 「解錠して編集」は本来意味を持たない（誤って書き換えると意図せずリンクが外れてしまう）。
/// 値を変えたい場合はトークン管理画面（`update_credential`）側で行う。
pub fn remask_cred_refs(conn: &Connection, raw_config_json: &str, visible_json: &str) -> Result<String> {
    let labels = cred_ref_labels(conn, raw_config_json)?;
    if labels.is_empty() {
        return Ok(visible_json.to_string());
    }
    let mut v: Value = serde_json::from_str(visible_json)?;
    if let Some(obj) = v.as_object_mut() {
        for key in labels.keys() {
            obj.insert(key.clone(), Value::String(secret::SECRET_PLACEHOLDER.to_string()));
        }
    }
    Ok(serde_json::to_string(&v)?)
}

/// ソースの config_json 内の `cred:<id>` 参照を、実際の値へ解決する
/// （connector 構築の直前でのみ呼ぶ。DB に保存する config_json 自体は変更しない）。
pub fn resolve_credentials(conn: &Connection, config_json: &str) -> Result<String> {
    let mut v: Value = serde_json::from_str(config_json)?;
    let Some(obj) = v.as_object_mut() else { return Ok(config_json.to_string()) };
    let refs: Vec<(String, i64)> = obj
        .iter()
        .filter_map(|(k, val)| {
            let s = val.as_str()?;
            let id_str = s.strip_prefix(secret::CRED_MARKER)?;
            let id: i64 = id_str.parse().ok()?;
            Some((k.clone(), id))
        })
        .collect();
    if refs.is_empty() {
        return Ok(config_json.to_string());
    }
    for (key, id) in refs {
        let fields_json: String = conn
            .query_row("SELECT fields_json FROM credentials WHERE id = ?1", [id], |r| r.get(0))
            .map_err(|_| anyhow!("保存済みトークン(id={id})が見つかりません（削除された可能性があります）"))?;
        let decrypted = secret::unprotect_config(&fields_json)?;
        let cred_v: Value = serde_json::from_str(&decrypted)?;
        if let Some(cred_obj) = cred_v.as_object() {
            if let Some(val) = cred_obj.get(&key) {
                obj.insert(key, val.clone());
            }
        }
    }
    Ok(serde_json::to_string(&v)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn scope_of_type_covers_all_secret_bearing_types() {
        assert_eq!(scope_of_type("notion"), Some("notion"));
        assert_eq!(scope_of_type("airtable"), Some("airtable"));
        assert_eq!(scope_of_type("postgres"), Some("postgres"));
        assert_eq!(scope_of_type("mysql"), Some("mysql"));
        assert_eq!(scope_of_type("box_files"), Some("box"));
        assert_eq!(scope_of_type("graph_entra_users"), Some("graph"));
        assert_eq!(scope_of_type("graph_sharepoint_list"), Some("graph"));
        assert_eq!(scope_of_type("csv"), None);
    }

    #[test]
    fn add_list_and_resolve_roundtrip() {
        let conn = open_test_db();
        let id = add_credential(&conn, "notion", "個人用", r#"{"token":"ntn_secret_abc"}"#).unwrap();

        let list = list_credentials(&conn, "notion").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].label, "個人用");
        assert!(list[0].used_by.is_empty());

        // resolve_credentials: cred:<id> が実値へ展開される。
        let cfg = serde_json::json!({ "token": format!("cred:{id}"), "database_id": "abc" }).to_string();
        let resolved = resolve_credentials(&conn, &cfg).unwrap();
        let v: Value = serde_json::from_str(&resolved).unwrap();
        assert_eq!(v["token"], "ntn_secret_abc");
        assert_eq!(v["database_id"], "abc");
    }

    #[test]
    fn public_fields_excludes_secret_key() {
        let conn = open_test_db();
        let id = add_credential(
            &conn,
            "graph",
            "テナントA",
            r#"{"tenant_id":"t1","client_id":"c1","client_secret":"s1"}"#,
        )
        .unwrap();
        let (label, fields) = public_fields(&conn, id).unwrap();
        assert_eq!(label, "テナントA");
        assert_eq!(fields.get("tenant_id").and_then(|v| v.as_str()), Some("t1"));
        assert_eq!(fields.get("client_id").and_then(|v| v.as_str()), Some("c1"));
        assert!(fields.get("client_secret").is_none(), "秘密フィールドはピッカーへ渡さない");
    }

    #[test]
    fn resolve_secret_value_returns_only_the_secret_field() {
        let conn = open_test_db();
        let id = add_credential(
            &conn,
            "graph",
            "テナントA",
            r#"{"tenant_id":"t1","client_id":"c1","client_secret":"s1"}"#,
        )
        .unwrap();
        assert_eq!(resolve_secret_value(&conn, id).unwrap(), "s1");
    }

    #[test]
    fn update_replaces_value_and_is_reflected_via_resolve() {
        let conn = open_test_db();
        let id = add_credential(&conn, "airtable", "業務用", r#"{"token":"pat_old"}"#).unwrap();

        update_credential(&conn, id, "業務用(改名)", r#"{"token":"pat_new"}"#).unwrap();

        let cfg = serde_json::json!({ "token": format!("cred:{id}") }).to_string();
        let resolved = resolve_credentials(&conn, &cfg).unwrap();
        let v: Value = serde_json::from_str(&resolved).unwrap();
        assert_eq!(v["token"], "pat_new"); // 共有参照方式: 置換が自動的に反映される

        let list = list_credentials(&conn, "airtable").unwrap();
        assert_eq!(list[0].label, "業務用(改名)");
    }

    #[test]
    fn update_keeps_old_secret_when_placeholder_untouched() {
        let conn = open_test_db();
        let id = add_credential(&conn, "notion", "A", r#"{"token":"ntn_original"}"#).unwrap();

        let placeholder_payload = serde_json::json!({ "token": secret::SECRET_PLACEHOLDER }).to_string();
        update_credential(&conn, id, "A(改名のみ)", &placeholder_payload).unwrap();

        let cfg = serde_json::json!({ "token": format!("cred:{id}") }).to_string();
        let resolved = resolve_credentials(&conn, &cfg).unwrap();
        let v: Value = serde_json::from_str(&resolved).unwrap();
        assert_eq!(v["token"], "ntn_original"); // プレースホルダーのままなら値は変わらない
    }

    #[test]
    fn delete_is_blocked_while_in_use_by_a_source() {
        let conn = open_test_db();
        let id = add_credential(&conn, "notion", "使用中トークン", r#"{"token":"ntn_x"}"#).unwrap();
        crate::db::insert_source(
            &conn,
            "notion",
            "議事録DB",
            &serde_json::json!({ "token": format!("cred:{id}"), "database_id": "abc" }).to_string(),
        )
        .unwrap();

        let err = delete_credential(&conn, id).unwrap_err();
        assert!(err.contains("議事録DB"), "使用中ソース名がエラーに含まれること: {err}");
        assert!(err.contains("1件"));

        // 実際には削除されていないこと。
        assert_eq!(list_credentials(&conn, "notion").unwrap().len(), 1);
    }

    #[test]
    fn delete_succeeds_when_unused() {
        let conn = open_test_db();
        let id = add_credential(&conn, "notion", "未使用トークン", r#"{"token":"ntn_y"}"#).unwrap();
        delete_credential(&conn, id).unwrap();
        assert!(list_credentials(&conn, "notion").unwrap().is_empty());
    }

    #[test]
    fn resolve_credentials_is_noop_when_no_cred_marker() {
        let conn = open_test_db();
        let cfg = serde_json::json!({ "token": "dpapi:aabb" }).to_string();
        let resolved = resolve_credentials(&conn, &cfg).unwrap();
        assert_eq!(resolved, cfg);
    }

    #[test]
    fn resolve_credentials_errors_when_referenced_credential_missing() {
        let conn = open_test_db();
        let cfg = serde_json::json!({ "token": "cred:9999" }).to_string();
        assert!(resolve_credentials(&conn, &cfg).is_err());
    }

    #[test]
    fn cred_ref_labels_finds_the_linked_field_and_its_label() {
        let conn = open_test_db();
        let id = add_credential(&conn, "notion", "共有トークン", r#"{"token":"ntn_x"}"#).unwrap();
        let cfg = serde_json::json!({ "token": format!("cred:{id}"), "database_id": "abc" }).to_string();
        let labels = cred_ref_labels(&conn, &cfg).unwrap();
        let token_entry = labels.get("token").unwrap();
        assert_eq!(token_entry["id"], id);
        assert_eq!(token_entry["label"], "共有トークン");
        assert!(labels.get("database_id").is_none());
    }

    #[test]
    fn remask_cred_refs_hides_value_even_if_visible_json_had_it_revealed() {
        // 解錠中に unprotect_config を通しても、cred: 参照フィールドは常にマスクされ直すこと。
        let conn = open_test_db();
        let id = add_credential(&conn, "notion", "共有トークン", r#"{"token":"ntn_x"}"#).unwrap();
        let raw = serde_json::json!({ "token": format!("cred:{id}") }).to_string();
        // 「もし unprotect_config が誤って何か実値を返してしまっていたら」を模した入力。
        let leaked_visible = serde_json::json!({ "token": "leaked_secret_value" }).to_string();
        let remasked = remask_cred_refs(&conn, &raw, &leaked_visible).unwrap();
        let v: Value = serde_json::from_str(&remasked).unwrap();
        assert_eq!(v["token"], secret::SECRET_PLACEHOLDER);
    }

    #[test]
    fn remask_cred_refs_is_noop_when_no_cred_marker() {
        let conn = open_test_db();
        let visible = serde_json::json!({ "token": "ntn_plain_secret" }).to_string();
        let out = remask_cred_refs(&conn, &visible, &visible).unwrap();
        assert_eq!(out, visible);
    }
}

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

pub const GRAPH_CAPABILITY_SHAREPOINT: &str = "sharepoint";
pub const GRAPH_CAPABILITY_CALENDAR_SELF: &str = "calendar_self";
pub const GRAPH_CAPABILITY_ONEDRIVE_SELF: &str = "onedrive_self";
const GRAPH_CAPABILITIES: &[&str] = &[
    GRAPH_CAPABILITY_SHAREPOINT,
    GRAPH_CAPABILITY_CALENDAR_SELF,
    GRAPH_CAPABILITY_ONEDRIVE_SELF,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphAuthMethod {
    AppOnly,
    Delegated,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GraphCredentialProfile {
    auth_method: GraphAuthMethod,
    capabilities: Vec<String>,
}

fn graph_profile(fields: &Value) -> Result<GraphCredentialProfile> {
    let auth_method = match fields
        .get("auth_method")
        .and_then(Value::as_str)
        .unwrap_or("app_only")
    {
        "app_only" => GraphAuthMethod::AppOnly,
        "delegated" => GraphAuthMethod::Delegated,
        value => {
            return Err(anyhow!(
                "Graph資格情報の認証方式が不正です: {value}（app_only または delegated を指定してください）"
            ))
        }
    };

    let capabilities = match fields.get("capabilities") {
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value.as_str().map(str::to_string).ok_or_else(|| {
                    anyhow!("Graph資格情報の capability は文字列の配列で指定してください")
                })
            })
            .collect::<Result<Vec<_>>>()?,
        Some(Value::Null) | None if auth_method == GraphAuthMethod::Delegated => {
            vec![GRAPH_CAPABILITY_SHAREPOINT.to_string()]
        }
        Some(Value::Null) | None => Vec::new(),
        Some(_) => {
            return Err(anyhow!(
                "Graph資格情報の capability は文字列の配列で指定してください"
            ))
        }
    };
    for capability in &capabilities {
        if !GRAPH_CAPABILITIES.contains(&capability.as_str()) {
            return Err(anyhow!(
                "Graph資格情報に未対応の capability が含まれています: {capability}"
            ));
        }
    }
    Ok(GraphCredentialProfile {
        auth_method,
        capabilities,
    })
}

fn delegated_capability_for_source(type_: &str) -> Option<&'static str> {
    match type_ {
        "graph_sharepoint_list" | "graph_sharepoint_library" => Some(GRAPH_CAPABILITY_SHAREPOINT),
        "graph_calendar" => Some(GRAPH_CAPABILITY_CALENDAR_SELF),
        "graph_onedrive" => Some(GRAPH_CAPABILITY_ONEDRIVE_SELF),
        _ => None,
    }
}

fn validate_graph_profile_for_source(
    type_: &str,
    source_name: Option<&str>,
    profile: &GraphCredentialProfile,
) -> Result<()> {
    if !type_.starts_with("graph") || profile.auth_method == GraphAuthMethod::AppOnly {
        return Ok(());
    }
    let source = source_name
        .map(|name| format!("「{name}」({type_})"))
        .unwrap_or_else(|| type_.to_string());
    let Some(required) = delegated_capability_for_source(type_) else {
        return Err(anyhow!(
            "Graph資格情報の互換性エラー: {source} は委任 OAuth に対応していません。アプリケーション認証を選択してください"
        ));
    };
    if !profile
        .capabilities
        .iter()
        .any(|capability| capability == required)
    {
        return Err(anyhow!(
            "Graph資格情報の互換性エラー: {source} には capability「{required}」が必要です。資格情報を再認可してください"
        ));
    }
    Ok(())
}

fn credential_record(conn: &Connection, id: i64) -> Result<(String, String)> {
    conn.query_row(
        "SELECT scope, fields_json FROM credentials WHERE id = ?1",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .map_err(|_| anyhow!("保存済みトークン(id={id})が見つかりません（削除された可能性があります）"))
}

fn credential_reference_id(config_json: &str, scope: &str) -> Result<Option<i64>> {
    let Some(key) = source_ref_key_of_scope(scope) else {
        return Ok(None);
    };
    let value: Value = serde_json::from_str(config_json)?;
    let Some(reference) = value.get(key).and_then(Value::as_str) else {
        return Ok(None);
    };
    let Some(id) = reference.strip_prefix(secret::CRED_MARKER) else {
        return Ok(None);
    };
    Ok(Some(id.parse().map_err(|_| {
        anyhow!("保存済みトークン参照が不正です: {reference}")
    })?))
}

/// ソース種別とリンクされた保存済み資格情報の認証方式・capabilityが適合するかを検証する。
/// 秘密値の復号やネットワークアクセスより前に呼び、誤設定を保存・実行させない。
pub fn validate_source_credential_compatibility(
    conn: &Connection,
    type_: &str,
    config_json: &str,
) -> Result<()> {
    let Some(expected_scope) = scope_of_type(type_) else {
        return Ok(());
    };
    let Some(id) = credential_reference_id(config_json, expected_scope)? else {
        return Ok(());
    };
    let (actual_scope, protected_fields) = credential_record(conn, id)?;
    if actual_scope != expected_scope {
        return Err(anyhow!(
            "保存済みトークンの種別が一致しません（期待: {expected_scope}、実際: {actual_scope}）"
        ));
    }
    if actual_scope == "graph" {
        let decrypted = secret::unprotect_config(&protected_fields)?;
        let fields: Value = serde_json::from_str(&decrypted)?;
        validate_graph_profile_for_source(type_, None, &graph_profile(&fields)?)?;
    }
    Ok(())
}

/// source type を伴う資格情報解決。Graphでは互換性検証を必須化する。
pub fn resolve_credentials_for_source_type(
    conn: &Connection,
    type_: &str,
    config_json: &str,
) -> Result<String> {
    validate_source_credential_compatibility(conn, type_, config_json)?;
    resolve_credentials(conn, config_json)
}

/// 対応スコープ（`sharedKeysFor` と同じ発想でフィールドをひとまとめにする単位）。
pub const SCOPES: &[&str] = &["notion", "airtable", "graph", "box", "postgres", "mysql"];

/// ソースの config_json 内で `cred:<id>` の生きた参照になる唯一のフィールド。
/// それ以外のフィールド（tenant_id 等）は選択時に平文コピーされるだけで、置換には追随しない。
///
/// box だけは特殊で、参照先は「credentials テーブルに保存された生の値」ではなく
/// `resolve_credentials` がその場で取得し直す一時的なアクセストークンになる
/// （Box は個人プランだと Client Credentials Grant が使えず、OAuth 2.0 のリフレッシュ
/// トークンから毎回アクセストークンを再取得する必要があるため。§7-17）。
fn source_ref_key_of_scope(scope: &str) -> Option<&'static str> {
    match scope {
        "notion" | "airtable" => Some("token"),
        "graph" => Some("client_secret"),
        "box" => Some("access_token"),
        "postgres" | "mysql" => Some("password"),
        _ => None,
    }
}

/// `credentials.fields_json` の中で暗号化保護すべきキー群（=ユーザーに平文で見せない値）。
/// box は client_secret（アプリのシークレット）と refresh_token（ユーザー本人の認可情報）の
/// 2つを持つ点が他スコープと異なる。
fn protected_keys_of_scope(scope: &str) -> &'static [&'static str] {
    match scope {
        "notion" | "airtable" => &["token"],
        "graph" => &["client_secret", "refresh_token"],
        "box" => &["client_secret", "refresh_token"],
        "postgres" | "mysql" => &["password"],
        _ => &[],
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
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut out = Vec::new();
    for (id, scope, label, created_at) in rows {
        let used_by = find_usage(conn, id)?
            .into_iter()
            .map(|(source_id, source_name)| CredentialUsage {
                source_id,
                source_name,
            })
            .collect();
        out.push(CredentialSummary {
            id,
            scope,
            label,
            created_at,
            used_by,
        });
    }
    Ok(out)
}

/// 新規保存。スコープの秘密フィールドだけ DPAPI 暗号化する。
pub fn add_credential(
    conn: &Connection,
    scope: &str,
    label: &str,
    fields_json: &str,
) -> Result<i64> {
    validate_scope(scope)?;
    if scope == "graph" {
        let fields: Value = serde_json::from_str(fields_json)?;
        graph_profile(&fields)?;
    }
    let protected = secret::protect_json_keys(protected_keys_of_scope(scope), fields_json)?;
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
    let keys = protected_keys_of_scope(&scope);
    let merged = secret::merge_preserving_placeholder_keys(keys, &old_fields_json, fields_json)?;
    if scope == "graph" {
        let decrypted = secret::unprotect_config(&merged)?;
        let fields: Value = serde_json::from_str(&decrypted)?;
        let profile = graph_profile(&fields)?;
        let mut stmt =
            conn.prepare("SELECT id, type, name, config_json FROM sources ORDER BY id")?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (_source_id, type_, source_name, config_json) in rows {
            if credential_reference_id(&config_json, "graph")? == Some(id) {
                validate_graph_profile_for_source(&type_, Some(&source_name), &profile)?;
            }
        }
    }
    let protected = secret::protect_json_keys(keys, &merged)?;
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
    conn.execute("DELETE FROM credentials WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 指定の保存済みトークンを参照している (source_id, source_name) の一覧。
pub fn find_usage(conn: &Connection, id: i64) -> Result<Vec<(i64, String)>> {
    let marker = format!("{}{id}", secret::CRED_MARKER);
    let mut stmt = conn.prepare("SELECT id, type, name, config_json FROM sources")?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut out = Vec::new();
    for (source_id, type_, name, config_json) in rows {
        let Some(scope) = scope_of_type(&type_) else {
            continue;
        };
        let Some(key) = source_ref_key_of_scope(scope) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<Value>(&config_json) else {
            continue;
        };
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
    let protected_keys = protected_keys_of_scope(&scope);
    let v: Value = serde_json::from_str(&fields_json)?;
    let mut out = Map::new();
    if let Some(obj) = v.as_object() {
        for (k, val) in obj {
            if protected_keys.contains(&k.as_str()) {
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
    // box / graph(delegated) の refresh_token はコピー用途に向かない。
    // graph は app_only の client_secret、または delegated でもコピー対象は client_secret キー
    // （空の場合あり）とする。
    let key = match scope.as_str() {
        "box" => "client_secret",
        "graph" => "client_secret",
        _ => source_ref_key_of_scope(&scope)
            .ok_or_else(|| anyhow!("秘密フィールドの無いスコープです: {scope}"))?,
    };
    let decrypted = secret::unprotect_config(&fields_json)?;
    let v: Value = serde_json::from_str(&decrypted)?;
    Ok(v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string())
}

/// 指定 config_json のうち、`cred:<id>` 参照になっているフィールドのキー→トークンのラベル。
/// マスターパスワードの解錠状態に関わらず呼べる（ラベルは秘密ではない。§7-3）。
/// 呼び出し側は主に 2 用途で使う: (1) UI に「🔗 使用中: <ラベル>」を表示する、
/// (2) 解錠時でも `cred:` 参照フィールドは実値を出さずマスクし直す（下記 `remask_cred_refs`）。
pub fn cred_ref_labels(conn: &Connection, config_json: &str) -> Result<Map<String, Value>> {
    let v: Value = serde_json::from_str(config_json)?;
    let mut out = Map::new();
    let Some(obj) = v.as_object() else {
        return Ok(out);
    };
    for (k, val) in obj {
        let Some(s) = val.as_str() else { continue };
        let Some(id_str) = s.strip_prefix(secret::CRED_MARKER) else {
            continue;
        };
        let Ok(id) = id_str.parse::<i64>() else {
            continue;
        };
        if let Ok(label) =
            conn.query_row("SELECT label FROM credentials WHERE id = ?1", [id], |r| {
                r.get::<_, String>(0)
            })
        {
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
pub fn remask_cred_refs(
    conn: &Connection,
    raw_config_json: &str,
    visible_json: &str,
) -> Result<String> {
    let labels = cred_ref_labels(conn, raw_config_json)?;
    if labels.is_empty() {
        return Ok(visible_json.to_string());
    }
    let mut v: Value = serde_json::from_str(visible_json)?;
    if let Some(obj) = v.as_object_mut() {
        for key in labels.keys() {
            obj.insert(
                key.clone(),
                Value::String(secret::SECRET_PLACEHOLDER.to_string()),
            );
        }
    }
    Ok(serde_json::to_string(&v)?)
}

/// ソースの config_json 内の `cred:<id>` 参照を、実際の値へ解決する
/// （connector 構築の直前でのみ呼ぶ。DB に保存する config_json 自体は変更しない）。
pub fn resolve_credentials(conn: &Connection, config_json: &str) -> Result<String> {
    let mut v: Value = serde_json::from_str(config_json)?;
    let Some(obj) = v.as_object_mut() else {
        return Ok(config_json.to_string());
    };
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
        let (scope, fields_json): (String, String) = conn
            .query_row(
                "SELECT scope, fields_json FROM credentials WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| {
                anyhow!("保存済みトークン(id={id})が見つかりません（削除された可能性があります）")
            })?;
        if scope == "box" {
            // box は credentials テーブルに「アクセストークン」自体を保持しない。個人プランは
            // OAuth 2.0（保存してある refresh_token からその場で取得し直し、ローテーションされる
            // 新しい refresh_token を保存し直す）、法人（Enterprise）プランは Client Credentials
            // Grant（その場で毎回新規発行。保存し直しは不要）のどちらかで取得する（§7-17, §7-18）。
            let access_token = box_access_token_for_credential(conn, id, &fields_json)?;
            obj.insert(key, Value::String(access_token));
            continue;
        }
        if scope == "graph" {
            let decrypted = secret::unprotect_config(&fields_json)?;
            let cred_v: Value = serde_json::from_str(&decrypted)?;
            let auth_method = cred_v
                .get("auth_method")
                .and_then(|x| x.as_str())
                .unwrap_or("app_only");
            let profile = graph_profile(&cred_v)?;
            obj.insert(
                "auth_method".to_string(),
                Value::String(
                    match profile.auth_method {
                        GraphAuthMethod::AppOnly => "app_only",
                        GraphAuthMethod::Delegated => "delegated",
                    }
                    .to_string(),
                ),
            );
            obj.insert(
                "capabilities".to_string(),
                Value::Array(
                    profile
                        .capabilities
                        .iter()
                        .cloned()
                        .map(Value::String)
                        .collect(),
                ),
            );
            if auth_method == "delegated" {
                let access_token = graph_access_token_for_credential(conn, id, &fields_json)?;
                // コネクタは access_token があれば client_credentials を使わない。
                obj.insert("access_token".to_string(), Value::String(access_token));
                obj.insert(key, Value::String(String::new()));
                if let Some(cred_obj) = cred_v.as_object() {
                    for copy_key in ["tenant_id", "client_id"] {
                        if obj
                            .get(copy_key)
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .is_empty()
                        {
                            if let Some(val) = cred_obj.get(copy_key) {
                                obj.insert(copy_key.to_string(), val.clone());
                            }
                        }
                    }
                }
                continue;
            }
            // app_only（既定）: client_secret を復号して埋め込む。
            if let Some(cred_obj) = cred_v.as_object() {
                if let Some(val) = cred_obj.get(&key) {
                    obj.insert(key, val.clone());
                }
            }
            continue;
        }
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

/// graph: `auth_method=delegated` のとき refresh_token からアクセストークンを取得する。
fn graph_access_token_for_credential(
    conn: &Connection,
    credential_id: i64,
    protected_fields_json: &str,
) -> Result<String> {
    let decrypted = secret::unprotect_config(protected_fields_json)?;
    let v: Value = serde_json::from_str(&decrypted)?;
    let tenant_id = v
        .get("tenant_id")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("テナントIDが未設定です"))?;
    let client_id = v
        .get("client_id")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("クライアントIDが未設定です"))?;
    let refresh_token = v
        .get("refresh_token")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            anyhow!(
                "Microsoft の認可が完了していません。トークン管理画面で「Microsoft にログインして認可」を行ってください"
            )
        })?;
    let (access_token, new_refresh_token) =
        crate::graph_oauth::refresh(tenant_id, client_id, refresh_token)?;
    if !new_refresh_token.is_empty() && new_refresh_token != refresh_token {
        let mut updated = v.clone();
        if let Some(obj) = updated.as_object_mut() {
            obj.insert(
                "refresh_token".to_string(),
                Value::String(new_refresh_token),
            );
        }
        let updated_json = serde_json::to_string(&updated)?;
        let reprotected =
            secret::protect_json_keys(protected_keys_of_scope("graph"), &updated_json)?;
        conn.execute(
            "UPDATE credentials SET fields_json = ?2 WHERE id = ?1",
            params![credential_id, reprotected],
        )?;
    }
    Ok(access_token)
}

/// 保存済み Graph 認証情報からピッカー用の認証引数を解決する。
pub fn resolve_graph_auth_args(
    conn: &Connection,
    id: i64,
    required_capability: &str,
) -> Result<crate::browse::GraphAuthArgs> {
    let (scope, fields_json): (String, String) = conn.query_row(
        "SELECT scope, fields_json FROM credentials WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if scope != "graph" {
        return Err(anyhow!("Graph 以外のトークンです: {scope}"));
    }
    let decrypted = secret::unprotect_config(&fields_json)?;
    let v: Value = serde_json::from_str(&decrypted)?;
    let tenant_id = v
        .get("tenant_id")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let client_id = v
        .get("client_id")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let auth_method = v
        .get("auth_method")
        .and_then(|x| x.as_str())
        .unwrap_or("app_only");
    let profile = graph_profile(&v)?;
    if profile.auth_method == GraphAuthMethod::Delegated
        && !profile
            .capabilities
            .iter()
            .any(|capability| capability == required_capability)
    {
        return Err(anyhow!(
            "Graph資格情報の互換性エラー: この操作には capability「{required_capability}」が必要です。資格情報を再認可してください"
        ));
    }
    if auth_method == "delegated" {
        let access_token = graph_access_token_for_credential(conn, id, &fields_json)?;
        Ok(crate::browse::GraphAuthArgs {
            tenant_id,
            client_id,
            client_secret: String::new(),
            access_token,
            auth_method: "delegated".to_string(),
        })
    } else {
        let client_secret = v
            .get("client_secret")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        Ok(crate::browse::GraphAuthArgs {
            tenant_id,
            client_id,
            client_secret,
            access_token: String::new(),
            auth_method: "app_only".to_string(),
        })
    }
}

/// box: `auth_method`（"oauth" 既定 / "ccg"）に応じてアクセストークンを取得する。
/// `protected_fields_json` は暗号化されたままの値でよい（内部で復号する）。
fn box_access_token_for_credential(
    conn: &Connection,
    credential_id: i64,
    protected_fields_json: &str,
) -> Result<String> {
    let decrypted = secret::unprotect_config(protected_fields_json)?;
    let v: Value = serde_json::from_str(&decrypted)?;
    let auth_method = v
        .get("auth_method")
        .and_then(|x| x.as_str())
        .unwrap_or("oauth");
    let client_id = v
        .get("client_id")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("クライアントIDが未設定です"))?;
    let client_secret = v
        .get("client_secret")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("クライアントシークレットが未設定です"))?;

    if auth_method == "ccg" {
        // 法人（Enterprise）プラン: Client Credentials Grant。毎回その場でトークンを新規発行する
        // だけで、保存し直す状態は無い（§7-18）。
        let enterprise_id = v
            .get("enterprise_id")
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("Enterprise ID が未設定です"))?;
        return crate::box_oauth::client_credentials_token(client_id, client_secret, enterprise_id);
    }

    // 個人プラン（既定）: OAuth 2.0。保存済み refresh_token からその場でアクセストークンを
    // 取得し直し、Box がローテーションする新しい refresh_token を保存し直す（§7-17）。
    let refresh_token = v.get("refresh_token").and_then(|x| x.as_str()).filter(|s| !s.is_empty()).ok_or_else(|| {
        anyhow!("Box の認可が完了していません。トークン管理画面で「Box にログインして認可」を行ってください")
    })?;
    let (access_token, new_refresh_token) =
        crate::box_oauth::refresh(client_id, client_secret, refresh_token)?;
    if !new_refresh_token.is_empty() {
        let mut updated = v.clone();
        if let Some(obj) = updated.as_object_mut() {
            obj.insert(
                "refresh_token".to_string(),
                Value::String(new_refresh_token),
            );
        }
        let updated_json = serde_json::to_string(&updated)?;
        let reprotected = secret::protect_json_keys(protected_keys_of_scope("box"), &updated_json)?;
        conn.execute(
            "UPDATE credentials SET fields_json = ?2 WHERE id = ?1",
            params![credential_id, reprotected],
        )?;
    }
    Ok(access_token)
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
        let id =
            add_credential(&conn, "notion", "個人用", r#"{"token":"ntn_secret_abc"}"#).unwrap();

        let list = list_credentials(&conn, "notion").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].label, "個人用");
        assert!(list[0].used_by.is_empty());

        // resolve_credentials: cred:<id> が実値へ展開される。
        let cfg =
            serde_json::json!({ "token": format!("cred:{id}"), "database_id": "abc" }).to_string();
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
        assert!(
            fields.get("client_secret").is_none(),
            "秘密フィールドはピッカーへ渡さない"
        );
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

        let placeholder_payload =
            serde_json::json!({ "token": secret::SECRET_PLACEHOLDER }).to_string();
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
        assert!(
            err.contains("議事録DB"),
            "使用中ソース名がエラーに含まれること: {err}"
        );
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
        let cfg =
            serde_json::json!({ "token": format!("cred:{id}"), "database_id": "abc" }).to_string();
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

    #[test]
    fn add_credential_protects_both_secret_keys_for_box() {
        // box は client_secret（アプリのシークレット）と refresh_token（ユーザー本人の認可情報）の
        // 2つとも平文で残してはいけない（§7-17: 個人プランは OAuth 2.0 のみ対応）。
        let conn = open_test_db();
        let id = add_credential(
            &conn,
            "box",
            "個人アカウント",
            r#"{"client_id":"cid","client_secret":"csec","refresh_token":"rtok"}"#,
        )
        .unwrap();
        let stored: String = conn
            .query_row(
                "SELECT fields_json FROM credentials WHERE id = ?1",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        let v: Value = serde_json::from_str(&stored).unwrap();
        assert_eq!(v["client_id"], "cid"); // 非秘密フィールドは平文のまま
        assert!(v["client_secret"].as_str().unwrap().starts_with("dpapi:"));
        assert!(v["refresh_token"].as_str().unwrap().starts_with("dpapi:"));
    }

    #[test]
    fn public_fields_excludes_graph_refresh_token() {
        let conn = open_test_db();
        let id = add_credential(
            &conn,
            "graph",
            "委任",
            r#"{"auth_method":"delegated","tenant_id":"t","client_id":"c","refresh_token":"rtok"}"#,
        )
        .unwrap();
        let (_label, fields) = public_fields(&conn, id).unwrap();
        assert_eq!(
            fields.get("auth_method").and_then(|v| v.as_str()),
            Some("delegated")
        );
        assert!(fields.get("refresh_token").is_none());
        assert!(fields.get("client_secret").is_none());
    }

    #[test]
    fn graph_profile_preserves_legacy_defaults() {
        let app_only = graph_profile(&serde_json::json!({})).unwrap();
        assert_eq!(app_only.auth_method, GraphAuthMethod::AppOnly);
        assert!(app_only.capabilities.is_empty());

        let delegated = graph_profile(&serde_json::json!({ "auth_method": "delegated" })).unwrap();
        assert_eq!(delegated.auth_method, GraphAuthMethod::Delegated);
        assert_eq!(delegated.capabilities, vec![GRAPH_CAPABILITY_SHAREPOINT]);
    }

    #[test]
    fn delegated_graph_sources_require_the_matching_capability() {
        let conn = open_test_db();
        let id = add_credential(
            &conn,
            "graph",
            "委任",
            r#"{"auth_method":"delegated","tenant_id":"t","client_id":"c","capabilities":["sharepoint"],"refresh_token":"rt"}"#,
        )
        .unwrap();
        let config = serde_json::json!({ "client_secret": format!("cred:{id}") }).to_string();

        validate_source_credential_compatibility(&conn, "graph_sharepoint_list", &config).unwrap();
        let error = validate_source_credential_compatibility(&conn, "graph_calendar", &config)
            .unwrap_err()
            .to_string();
        assert!(error.contains("calendar_self"));
        let error = validate_source_credential_compatibility(&conn, "graph_teams", &config)
            .unwrap_err()
            .to_string();
        assert!(error.contains("委任 OAuth に対応していません"));
    }

    #[test]
    fn app_only_graph_credentials_remain_compatible_with_all_graph_sources() {
        let conn = open_test_db();
        let id = add_credential(
            &conn,
            "graph",
            "アプリ",
            r#"{"tenant_id":"t","client_id":"c","client_secret":"s"}"#,
        )
        .unwrap();
        let config = serde_json::json!({ "client_secret": format!("cred:{id}") }).to_string();
        for type_ in [
            "graph_entra_users",
            "graph_teams",
            "graph_sharepoint_library",
            "graph_calendar",
            "graph_onedrive",
            "graph_planner",
        ] {
            validate_source_credential_compatibility(&conn, type_, &config).unwrap();
        }
    }

    #[test]
    fn credential_update_is_rejected_before_breaking_a_linked_source() {
        let conn = open_test_db();
        let id = add_credential(
            &conn,
            "graph",
            "アプリ",
            r#"{"auth_method":"app_only","tenant_id":"t","client_id":"c","client_secret":"s"}"#,
        )
        .unwrap();
        crate::db::insert_source(
            &conn,
            "graph_teams",
            "全社Teams",
            &serde_json::json!({ "client_secret": format!("cred:{id}") }).to_string(),
        )
        .unwrap();

        let error = update_credential(
            &conn,
            id,
            "委任へ変更",
            r#"{"auth_method":"delegated","tenant_id":"t","client_id":"c","capabilities":["sharepoint"],"refresh_token":"rt"}"#,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("全社Teams"));
        assert!(error.contains("委任 OAuth に対応していません"));

        let (_, fields) = public_fields(&conn, id).unwrap();
        assert_eq!(
            fields.get("auth_method").and_then(Value::as_str),
            Some("app_only")
        );
    }

    #[test]
    fn graph_browse_rejects_missing_capability_before_token_refresh() {
        let conn = open_test_db();
        let id = add_credential(
            &conn,
            "graph",
            "予定表のみ",
            r#"{"auth_method":"delegated","tenant_id":"t","client_id":"c","capabilities":["calendar_self"],"refresh_token":"not-used"}"#,
        )
        .unwrap();
        let error = resolve_graph_auth_args(&conn, id, GRAPH_CAPABILITY_SHAREPOINT)
            .unwrap_err()
            .to_string();
        assert!(error.contains("sharepoint"));
    }

    #[test]
    fn protected_keys_of_graph_include_refresh_token() {
        assert!(protected_keys_of_scope("graph").contains(&"refresh_token"));
        assert!(protected_keys_of_scope("graph").contains(&"client_secret"));
    }

    #[test]
    fn public_fields_excludes_both_protected_keys_for_box() {
        let conn = open_test_db();
        let id = add_credential(
            &conn,
            "box",
            "個人アカウント",
            r#"{"client_id":"cid","client_secret":"csec","refresh_token":"rtok"}"#,
        )
        .unwrap();
        let (label, fields) = public_fields(&conn, id).unwrap();
        assert_eq!(label, "個人アカウント");
        assert_eq!(
            fields.get("client_id").and_then(|v| v.as_str()),
            Some("cid")
        );
        assert!(fields.get("client_secret").is_none());
        assert!(fields.get("refresh_token").is_none());
    }

    #[test]
    fn resolve_credentials_for_box_fails_clearly_when_not_yet_authorized() {
        // refresh_token が無い（＝トークン管理画面でまだ「Box にログインして認可」していない）
        // 状態で使おうとしたときは、ネットワークへは出ずに分かりやすいエラーを返す。
        let conn = open_test_db();
        let id = add_credential(
            &conn,
            "box",
            "個人アカウント",
            r#"{"client_id":"cid","client_secret":"csec"}"#,
        )
        .unwrap();
        let cfg = serde_json::json!({ "access_token": format!("cred:{id}"), "folder_id": "0" })
            .to_string();
        let err = resolve_credentials(&conn, &cfg).unwrap_err();
        assert!(
            err.to_string().contains("認可"),
            "未認可時のエラーメッセージであること: {err}"
        );
    }
}

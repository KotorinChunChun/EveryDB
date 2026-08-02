//! 認証済みクラウドサービスを取得する connector 群。

use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::Value;

use super::common::{make_record, CommonCfg};
use super::limits::{self, static_field_names, API_PAGE_SIZE};
use super::Connector;
use crate::connectors::{col_to_letters, default_true, json_scalar_to_string, urlencoding_light};
use crate::model::Record;
// ---- Google カレンダー / iCal（公開 .ics URL。認証不要）----
pub(crate) fn ical_unescape(s: &str) -> String {
    s.replace(r"\n", "\n")
        .replace(r"\N", "\n")
        .replace(r"\,", ",")
        .replace(r"\;", ";")
        .replace(r"\\", "\\")
}

/// "20260101T090000Z" / "20260101T090000" / "20260101"(終日) を表示用文字列へ。
pub(crate) fn parse_ical_datetime(v: &str) -> String {
    let digits: String = v
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == 'T')
        .collect();
    if digits.len() >= 15 && digits.as_bytes()[8] == b'T' {
        format!(
            "{}-{}-{} {}:{}:{}",
            &digits[0..4],
            &digits[4..6],
            &digits[6..8],
            &digits[9..11],
            &digits[11..13],
            &digits[13..15]
        )
    } else if digits.len() >= 8 {
        format!("{}-{}-{}", &digits[0..4], &digits[4..6], &digits[6..8])
    } else {
        v.to_string()
    }
}

/// RFC5545 の行折り返し(継続行は空白/タブで始まる)を1行に戻す。
pub(crate) fn ical_unfold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.replace("\r\n", "\n").lines() {
        if (line.starts_with(' ') || line.starts_with('\t')) && !out.is_empty() {
            out.push_str(line.trim_start_matches([' ', '\t']));
        } else {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(line);
        }
    }
    out
}

#[derive(Deserialize)]
pub(crate) struct IcalConnector {
    url: String,
    #[serde(flatten)]
    common: CommonCfg,
}

impl Connector for IcalConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let resp = limits::http_get(&self.url);
        let text = match resp {
            Ok(r) => r.into_string()?,
            Err(e) => return Err(anyhow!("iCal 取得失敗: {e}")),
        };
        let unfolded = ical_unfold(&text);

        let mut out = Vec::new();
        let mut cur: Option<std::collections::HashMap<String, String>> = None;
        for line in unfolded.lines() {
            match line {
                "BEGIN:VEVENT" => cur = Some(std::collections::HashMap::new()),
                "END:VEVENT" => {
                    if let Some(map) = cur.take() {
                        let summary = map.get("SUMMARY").cloned().unwrap_or_default();
                        let pairs = vec![
                            ("件名".to_string(), summary.clone()),
                            (
                                "開始".to_string(),
                                map.get("DTSTART").cloned().unwrap_or_default(),
                            ),
                            (
                                "終了".to_string(),
                                map.get("DTEND").cloned().unwrap_or_default(),
                            ),
                            (
                                "場所".to_string(),
                                map.get("LOCATION").cloned().unwrap_or_default(),
                            ),
                            (
                                "詳細".to_string(),
                                map.get("DESCRIPTION").cloned().unwrap_or_default(),
                            ),
                        ];
                        let mut rec = make_record(pairs, &self.url, &self.common);
                        if self.common.label_column.is_none() {
                            rec.label = Some(summary);
                        }
                        out.push(rec);
                    }
                }
                _ => {
                    if let (Some(map), Some(idx)) = (cur.as_mut(), line.find(':')) {
                        let key = line[..idx].split(';').next().unwrap_or("").to_uppercase();
                        let raw_value = ical_unescape(&line[idx + 1..]);
                        let value = if key == "DTSTART" || key == "DTEND" {
                            parse_ical_datetime(&raw_value)
                        } else {
                            raw_value
                        };
                        map.insert(key, value);
                    }
                }
            }
        }
        Ok(out)
    }

    fn field_names(&self) -> Result<Vec<String>> {
        static_field_names(&["件名", "開始", "終了", "場所", "詳細"])
    }
}

// ---- Airtable（REST API + Personal Access Token）----
pub(crate) fn airtable_value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(a) => a
            .iter()
            .map(|item| match item {
                Value::String(s) => s.clone(),
                Value::Object(o) => o
                    .get("filename")
                    .or_else(|| o.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                other => airtable_value_to_string(other),
            })
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    }
}

#[derive(Deserialize)]
pub(crate) struct AirtableConnector {
    token: String,
    base_id: String,
    /// テーブルの ID(tbl...) または名前。
    table: String,
    /// ビュー ID/名前（任意。指定するとそのビューの絞り込み/並び順が反映される）。
    #[serde(default)]
    view: Option<String>,
    #[serde(flatten)]
    common: CommonCfg,
}

impl Connector for AirtableConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let mut out = Vec::new();
        let mut offset: Option<String> = None;
        loop {
            let mut url = format!(
                "https://api.airtable.com/v0/{}/{}?pageSize={}",
                urlencoding_light(&self.base_id),
                urlencoding_light(&self.table),
                API_PAGE_SIZE
            );
            if let Some(v) = &self.view {
                url.push_str(&format!("&view={}", urlencoding_light(v)));
            }
            if let Some(o) = &offset {
                url.push_str(&format!("&offset={}", urlencoding_light(o)));
            }
            let resp = limits::http_get_bearer(&url, &self.token);
            let body: Value = match resp {
                Ok(r) => r.into_json()?,
                Err(e) => return Err(anyhow!("Airtable API: {e}")),
            };
            if let Some(records) = body.get("records").and_then(|v| v.as_array()) {
                for rec in records {
                    let id = rec
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let mut pairs: Vec<(String, String)> = rec
                        .get("fields")
                        .and_then(|v| v.as_object())
                        .map(|o| {
                            o.iter()
                                .map(|(k, v)| (k.clone(), airtable_value_to_string(v)))
                                .collect()
                        })
                        .unwrap_or_default();
                    pairs.push(("レコードID".to_string(), id.clone()));
                    let open_ref = format!(
                        "https://airtable.com/{}/{}/{}",
                        self.base_id, self.table, id
                    );
                    limits::push_record(
                        &mut out,
                        make_record(pairs, &open_ref, &self.common),
                        "Airtable",
                    )?;
                }
            }
            limits::check_fetch_limit(out.len(), "Airtable")?;
            offset = body
                .get("offset")
                .and_then(|v| v.as_str())
                .map(String::from);
            if offset.is_none() {
                break;
            }
        }
        Ok(out)
    }

    fn field_names(&self) -> Result<Vec<String>> {
        let mut url = format!(
            "https://api.airtable.com/v0/{}/{}?pageSize=1&maxRecords=1",
            urlencoding_light(&self.base_id),
            urlencoding_light(&self.table)
        );
        if let Some(v) = &self.view {
            url.push_str(&format!("&view={}", urlencoding_light(v)));
        }
        let body: Value = limits::http_get_bearer(&url, &self.token)?.into_json()?;
        let mut names = body
            .get("records")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|rec| rec.get("fields"))
            .and_then(|v| v.as_object())
            .map(|o| o.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        names.push("レコードID".to_string());
        Ok(names)
    }
}

// ---- Notion データベース（要件 M6 / REST + インテグレーショントークン）----
#[derive(Deserialize)]
pub(crate) struct NotionConnector {
    token: String,
    database_id: String,
    #[serde(flatten)]
    common: CommonCfg,
}

const NOTION_VERSION: &str = "2022-06-28";

/// Notion のプロパティ値 1 個を表示用文字列へ変換する（型ごとに分岐）。
pub(crate) fn notion_prop_to_string(p: &Value) -> String {
    let t = p.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let rich = |arr: Option<&Value>| -> String {
        arr.and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.get("plain_text").and_then(|v| v.as_str()))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default()
    };
    match t {
        "title" | "rich_text" => rich(p.get(t)),
        "number" => match p.get("number") {
            Some(Value::Number(n)) => n.to_string(),
            _ => String::new(),
        },
        "select" | "status" => p
            .get(t)
            .and_then(|s| s.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "multi_select" => p
            .get("multi_select")
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.get("name").and_then(|v| v.as_str()))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default(),
        "date" => {
            let d = p.get("date");
            let start = d
                .and_then(|x| x.get("start"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let end = d.and_then(|x| x.get("end")).and_then(|v| v.as_str());
            match end {
                Some(e) if !e.is_empty() => format!("{start} ~ {e}"),
                _ => start.to_string(),
            }
        }
        "checkbox" => match p.get("checkbox").and_then(|v| v.as_bool()) {
            Some(true) => "✓".to_string(),
            _ => String::new(),
        },
        "url" | "email" | "phone_number" | "created_time" | "last_edited_time" => {
            p.get(t).and_then(|v| v.as_str()).unwrap_or("").to_string()
        }
        "people" => p
            .get("people")
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.get("name").and_then(|v| v.as_str()))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default(),
        "files" => p
            .get("files")
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.get("name").and_then(|v| v.as_str()))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default(),
        "formula" => {
            let f = p.get("formula");
            let ft = f
                .and_then(|x| x.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match ft {
                "string" => f
                    .and_then(|x| x.get("string"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                "number" => f
                    .and_then(|x| x.get("number"))
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                "boolean" => match f.and_then(|x| x.get("boolean")).and_then(|v| v.as_bool()) {
                    Some(true) => "✓".to_string(),
                    _ => String::new(),
                },
                "date" => f
                    .and_then(|x| x.get("date"))
                    .and_then(|x| x.get("start"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                _ => String::new(),
            }
        }
        "relation" => {
            let n = p
                .get("relation")
                .and_then(|a| a.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            if n > 0 {
                format!("{n} 件")
            } else {
                String::new()
            }
        }
        "unique_id" => {
            let num = p.get("unique_id").and_then(|x| x.get("number"));
            let prefix = p
                .get("unique_id")
                .and_then(|x| x.get("prefix"))
                .and_then(|v| v.as_str());
            match (prefix, num) {
                (Some(pre), Some(n)) => format!("{pre}-{n}"),
                (None, Some(n)) => n.to_string(),
                _ => String::new(),
            }
        }
        _ => String::new(),
    }
}

/// Notion のページ 1 件（properties + url）を共通 Record へ正規化する。
pub(crate) fn notion_page_to_record(page: &Value, common: &CommonCfg) -> Record {
    let page_url = page.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let props = page.get("properties").and_then(|v| v.as_object());
    let mut pairs: Vec<(String, String)> = Vec::new();
    if let Some(props) = props {
        for (name, p) in props {
            pairs.push((name.clone(), notion_prop_to_string(p)));
        }
    }
    let mut rec = make_record(pairs, page_url, common);
    // label 未指定なら title 型プロパティを主見出しにする（Notion 特有）。
    if common.label_column.is_none() {
        if let Some(props) = props {
            for (_name, p) in props {
                if p.get("type").and_then(|v| v.as_str()) == Some("title") {
                    rec.label = Some(notion_prop_to_string(p));
                    break;
                }
            }
        }
    }
    rec
}

impl Connector for NotionConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let url = format!(
            "https://api.notion.com/v1/databases/{}/query",
            self.database_id
        );
        let mut out = Vec::new();
        let mut start_cursor: Option<String> = None;

        loop {
            let mut body = serde_json::json!({ "page_size": API_PAGE_SIZE });
            if let Some(c) = &start_cursor {
                body["start_cursor"] = Value::String(c.clone());
            }
            let resp = limits::http_post_json_with_headers(
                &url,
                &[
                    ("Authorization", &format!("Bearer {}", self.token)),
                    ("Notion-Version", NOTION_VERSION),
                    ("Content-Type", "application/json"),
                ],
                body,
            );

            let value: Value = match resp {
                Ok(r) => r.into_json()?,
                Err(e) => return Err(anyhow!("Notion API: {e}")),
            };

            if let Some(results) = value.get("results").and_then(|v| v.as_array()) {
                for page in results {
                    limits::push_record(
                        &mut out,
                        notion_page_to_record(page, &self.common),
                        "Notion",
                    )?;
                }
            }
            limits::check_fetch_limit(out.len(), "Notion")?;

            let has_more = value
                .get("has_more")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            start_cursor = value
                .get("next_cursor")
                .and_then(|v| v.as_str())
                .map(String::from);
            if !has_more || start_cursor.is_none() {
                break;
            }
        }
        Ok(out)
    }

    fn field_names(&self) -> Result<Vec<String>> {
        let url = format!("https://api.notion.com/v1/databases/{}", self.database_id);
        let value: Value = limits::http_get_with_headers(
            &url,
            &[
                ("Authorization", &format!("Bearer {}", self.token)),
                ("Notion-Version", NOTION_VERSION),
            ],
        )?
        .into_json()?;
        Ok(value
            .get("properties")
            .and_then(|v| v.as_object())
            .map(|props| props.keys().cloned().collect())
            .unwrap_or_default())
    }
}

// ---- Google Spreadsheet（サービスアカウント JWT 認証）----
#[derive(serde::Serialize)]
struct GoogleJwtClaims {
    iss: String,
    scope: String,
    aud: String,
    exp: i64,
    iat: i64,
}

/// サービスアカウント JSON 鍵（の中身の文字列）から Google API アクセストークンを取得する。
fn google_access_token(sa_json: &str, scope: &str) -> Result<String> {
    let sa: Value = serde_json::from_str(sa_json)
        .map_err(|e| anyhow!("サービスアカウント JSON の解析に失敗: {e}"))?;
    let client_email = sa
        .get("client_email")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("client_email が見つかりません"))?;
    let private_key = sa
        .get("private_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("private_key が見つかりません"))?;
    let token_uri = sa
        .get("token_uri")
        .and_then(|v| v.as_str())
        .unwrap_or("https://oauth2.googleapis.com/token");

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;
    let claims = GoogleJwtClaims {
        iss: client_email.to_string(),
        scope: scope.to_string(),
        aud: token_uri.to_string(),
        exp: now + 3600,
        iat: now,
    };
    let key = jsonwebtoken::EncodingKey::from_rsa_pem(private_key.as_bytes())
        .map_err(|e| anyhow!("秘密鍵の読み込みに失敗: {e}"))?;
    let jwt = jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256),
        &claims,
        &key,
    )
    .map_err(|e| anyhow!("JWT 署名に失敗: {e}"))?;

    let resp = limits::http_post_form(
        token_uri,
        &[
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("assertion", &jwt),
        ],
    );
    let body: Value = match resp {
        Ok(r) => r.into_json()?,
        Err(e) => return Err(anyhow!("Google トークン取得失敗: {e}")),
    };
    body.get("access_token")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| anyhow!("access_token が返りませんでした"))
}

fn google_get_json(url: &str, token: &str) -> Result<Value> {
    let resp = limits::http_get_bearer(url, token).map_err(|e| anyhow!("Google API: {e}"))?;
    resp.into_json()
        .map_err(|e| anyhow!("Google API JSON: {e}"))
}

#[derive(Deserialize)]
pub(crate) struct GoogleSheetsConnector {
    /// サービスアカウント JSON 鍵ファイルへのパス（ダウンロードした .json をそのまま指す）。
    key_path: String,
    spreadsheet_id: String,
    /// "Sheet1" や "Sheet1!A1:D50" 形式。省略時は先頭シートの全域。
    #[serde(default)]
    range: Option<String>,
    #[serde(default = "default_true")]
    has_header: bool,
    #[serde(flatten)]
    common: CommonCfg,
}

impl Connector for GoogleSheetsConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let sa_json = std::fs::read_to_string(&self.key_path)
            .map_err(|e| anyhow!("サービスアカウント鍵ファイルの読み込みに失敗: {e}"))?;
        let token = google_access_token(
            &sa_json,
            "https://www.googleapis.com/auth/spreadsheets.readonly",
        )?;

        let range = match &self.range {
            Some(r) if !r.is_empty() => r.clone(),
            _ => {
                let meta_url = format!(
                    "https://sheets.googleapis.com/v4/spreadsheets/{}?fields=sheets.properties.title",
                    self.spreadsheet_id
                );
                let meta = google_get_json(&meta_url, &token)?;
                meta.get("sheets")
                    .and_then(|s| s.as_array())
                    .and_then(|a| a.first())
                    .and_then(|s| s.get("properties"))
                    .and_then(|p| p.get("title"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        anyhow!("シート情報を取得できません（共有設定を確認してください）")
                    })?
                    .to_string()
            }
        };

        let values_url = format!(
            "https://sheets.googleapis.com/v4/spreadsheets/{}/values/{}",
            self.spreadsheet_id,
            urlencoding_light(&range)
        );
        let data = google_get_json(&values_url, &token)?;
        let rows_2d = data
            .get("values")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let cell_str = |v: &Value| -> String {
            match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => String::new(),
            }
        };

        let mut iter = rows_2d.iter();
        let headers: Vec<String> = if self.has_header {
            iter.next()
                .and_then(|r| r.as_array())
                .map(|r| r.iter().map(cell_str).collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let mut out = Vec::new();
        for row in iter {
            let cells = row.as_array().cloned().unwrap_or_default();
            if cells.is_empty() {
                continue;
            }
            let ncol = if self.has_header {
                headers.len().max(cells.len())
            } else {
                cells.len()
            };
            let pairs: Vec<(String, String)> = (0..ncol)
                .map(|c| {
                    let name = if self.has_header {
                        headers
                            .get(c)
                            .filter(|h| !h.is_empty())
                            .cloned()
                            .unwrap_or_else(|| col_to_letters(c as u32))
                    } else {
                        col_to_letters(c as u32)
                    };
                    let val = cells.get(c).map(cell_str).unwrap_or_default();
                    (name, val)
                })
                .collect();
            limits::push_record(
                &mut out,
                make_record(pairs, "", &self.common),
                "Google Sheets",
            )?;
        }
        Ok(out)
    }

    fn field_names(&self) -> Result<Vec<String>> {
        let sa_json = std::fs::read_to_string(&self.key_path)
            .map_err(|e| anyhow!("サービスアカウント鍵ファイルの読み込みに失敗: {e}"))?;
        let token = google_access_token(
            &sa_json,
            "https://www.googleapis.com/auth/spreadsheets.readonly",
        )?;

        let range = match &self.range {
            Some(r) if !r.is_empty() => {
                if r.contains('!') {
                    format!("{}!1:1", r.split('!').next().unwrap_or(r))
                } else {
                    format!("{r}!1:1")
                }
            }
            _ => {
                let meta_url = format!(
                    "https://sheets.googleapis.com/v4/spreadsheets/{}?fields=sheets.properties.title",
                    self.spreadsheet_id
                );
                let meta = google_get_json(&meta_url, &token)?;
                let sheet = meta
                    .get("sheets")
                    .and_then(|s| s.as_array())
                    .and_then(|a| a.first())
                    .and_then(|s| s.get("properties"))
                    .and_then(|p| p.get("title"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        anyhow!("シート情報を取得できません（共有設定を確認してください）")
                    })?;
                format!("{sheet}!1:1")
            }
        };

        let values_url = format!(
            "https://sheets.googleapis.com/v4/spreadsheets/{}/values/{}",
            self.spreadsheet_id,
            urlencoding_light(&range)
        );
        let data = google_get_json(&values_url, &token)?;
        let row = data
            .get("values")
            .and_then(|v| v.as_array())
            .and_then(|rows| rows.first())
            .and_then(|r| r.as_array());

        let cell_str = |v: &Value| -> String {
            match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => String::new(),
            }
        };

        if self.has_header {
            Ok(row
                .map(|r| r.iter().map(cell_str).filter(|h| !h.is_empty()).collect())
                .unwrap_or_default())
        } else {
            Ok(row
                .map(|r| (0..r.len()).map(|c| col_to_letters(c as u32)).collect())
                .unwrap_or_default())
        }
    }
}

// ---- Box（OAuth 2.0。要件: クラウドストレージ系。§7-17 個人プランは Client Credentials
//      Grant が使えないため、access_token は commands::credentials::resolve_credentials 側で
//      Box にログイン済みの refresh_token から解決済みのものがここに渡ってくる）----
#[derive(Deserialize)]
pub(crate) struct BoxConnector {
    access_token: String,
    /// 走査開始フォルダ ID（省略時はルート "0"）。
    #[serde(default = "default_box_folder")]
    folder_id: String,
    #[serde(flatten)]
    common: CommonCfg,
}
fn default_box_folder() -> String {
    "0".to_string()
}

impl Connector for BoxConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let mut out = Vec::new();
        self.walk_box_folder(&self.access_token, &self.folder_id, "", &mut out)?;
        Ok(out)
    }

    fn field_names(&self) -> Result<Vec<String>> {
        static_field_names(&["名前", "種類", "サイズ", "更新日時", "パス"])
    }
}

impl BoxConnector {
    fn walk_box_folder(
        &self,
        token: &str,
        folder_id: &str,
        path: &str,
        out: &mut Vec<Record>,
    ) -> Result<()> {
        let mut offset = 0u32;
        loop {
            let url = format!(
                "https://api.box.com/2.0/folders/{folder_id}/items?fields=name,type,size,modified_at,path_collection&limit={}&offset={offset}",
                API_PAGE_SIZE
            );
            let resp = limits::http_get_bearer(&url, token);
            let body: Value = match resp {
                Ok(r) => r.into_json()?,
                Err(e) => return Err(anyhow!("Box API: {e}")),
            };
            let entries = body
                .get("entries")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            if entries.is_empty() {
                break;
            }
            for entry in &entries {
                let name = entry
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let item_type = entry.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let id = entry
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let size = entry
                    .get("size")
                    .map(json_scalar_to_string)
                    .unwrap_or_default();
                let modified = entry
                    .get("modified_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let web_url = if item_type == "folder" {
                    format!("https://app.box.com/folder/{id}")
                } else {
                    format!("https://app.box.com/file/{id}")
                };
                let pairs = vec![
                    ("名前".to_string(), name.clone()),
                    (
                        "種類".to_string(),
                        if item_type == "folder" {
                            "フォルダ"
                        } else {
                            "ファイル"
                        }
                        .to_string(),
                    ),
                    ("サイズ".to_string(), size),
                    ("更新日時".to_string(), modified),
                    ("パス".to_string(), path.to_string()),
                ];
                limits::push_record(
                    out,
                    {
                        let mut rec = make_record(pairs, &web_url, &self.common);
                        if self.common.label_column.is_none() {
                            rec.label = Some(name.clone());
                        }
                        rec
                    },
                    "Box",
                )?;

                if item_type == "folder" {
                    let new_path = if path.is_empty() {
                        name.clone()
                    } else {
                        format!("{path}/{name}")
                    };
                    self.walk_box_folder(token, &id, &new_path, out)?;
                }
            }
            let total = body
                .get("total_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            offset += entries.len() as u32;
            if offset >= total {
                break;
            }
        }
        Ok(())
    }
}

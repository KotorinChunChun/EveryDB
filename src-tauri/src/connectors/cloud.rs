//! 認証済みクラウドサービスを取得する connector 群。

use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::Value;

use super::common::{make_record, CommonCfg};
use super::Connector;
use crate::connectors::urlencoding_light;
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
        let resp = ureq::get(&self.url).call();
        let text = match resp {
            Ok(r) => r.into_string()?,
            Err(ureq::Error::Status(code, r)) => {
                return Err(anyhow!(
                    "iCal 取得失敗 {code}: {}",
                    r.into_string().unwrap_or_default()
                ))
            }
            Err(e) => return Err(anyhow!("iCal URL への接続に失敗: {e}")),
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
                "https://api.airtable.com/v0/{}/{}?pageSize=100",
                urlencoding_light(&self.base_id),
                urlencoding_light(&self.table)
            );
            if let Some(v) = &self.view {
                url.push_str(&format!("&view={}", urlencoding_light(v)));
            }
            if let Some(o) = &offset {
                url.push_str(&format!("&offset={}", urlencoding_light(o)));
            }
            let resp = ureq::get(&url)
                .set("Authorization", &format!("Bearer {}", self.token))
                .call();
            let body: Value = match resp {
                Ok(r) => r.into_json()?,
                Err(ureq::Error::Status(code, r)) => {
                    return Err(anyhow!(
                        "Airtable API エラー {code}: {}",
                        r.into_string().unwrap_or_default()
                    ))
                }
                Err(e) => return Err(anyhow!("Airtable への接続に失敗: {e}")),
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
                    out.push(make_record(pairs, &open_ref, &self.common));
                }
            }
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
}

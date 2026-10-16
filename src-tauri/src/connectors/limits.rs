//! connector 共通の HTTP タイムアウト・ページサイズ・取得件数上限。

use anyhow::{anyhow, Result};
use std::sync::OnceLock;
use std::time::Duration;

use crate::model::Record;

/// HTTP 接続・読み取りのタイムアウト（秒）。
pub const HTTP_TIMEOUT_SECS: u64 = 30;
/// 1 回の fetch で保持するレコード数の上限。
pub const MAX_FETCH_RECORDS: usize = 100_000;
/// REST API の 1 ページあたり件数（Airtable / Notion / Box 等）。
pub const API_PAGE_SIZE: u32 = 100;
/// Microsoft Graph の `$top` 既定値。
pub const GRAPH_PAGE_TOP: u32 = 999;
/// SQL connector が 1 回で読む行数上限。
pub const DB_FETCH_LIMIT: i64 = 20_000;

static HTTP_AGENT: OnceLock<ureq::Agent> = OnceLock::new();

pub fn http_agent() -> &'static ureq::Agent {
    HTTP_AGENT.get_or_init(|| {
        ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .timeout_read(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .build()
    })
}

fn map_http_error(method: &str, e: ureq::Error) -> anyhow::Error {
    match e {
        ureq::Error::Status(code, r) => anyhow!(
            "HTTP {method} {code}: {}",
            r.into_string().unwrap_or_default()
        ),
        e => anyhow!("HTTP {method} への接続に失敗: {e}"),
    }
}

pub fn http_get(url: &str) -> Result<ureq::Response> {
    http_agent()
        .get(url)
        .call()
        .map_err(|e| map_http_error("GET", e))
}

pub fn http_get_with_headers(url: &str, headers: &[(&str, &str)]) -> Result<ureq::Response> {
    let mut req = http_agent().get(url);
    for (k, v) in headers {
        req = req.set(k, v);
    }
    req.call().map_err(|e| map_http_error("GET", e))
}

pub fn http_get_bearer(url: &str, token: &str) -> Result<ureq::Response> {
    http_agent()
        .get(url)
        .set("Authorization", &format!("Bearer {token}"))
        .call()
        .map_err(|e| map_http_error("GET", e))
}

pub fn http_post_form(url: &str, form: &[(&str, &str)]) -> Result<ureq::Response> {
    http_agent()
        .post(url)
        .send_form(form)
        .map_err(|e| map_http_error("POST", e))
}

pub fn http_post_json_with_headers(
    url: &str,
    headers: &[(&str, &str)],
    body: serde_json::Value,
) -> Result<ureq::Response> {
    let mut req = http_agent().post(url);
    for (k, v) in headers {
        req = req.set(k, v);
    }
    req.send_json(body).map_err(|e| map_http_error("POST", e))
}

pub fn check_fetch_limit(count: usize, context: &str) -> Result<()> {
    if count > MAX_FETCH_RECORDS {
        Err(anyhow!(
            "{context}: 取得件数が上限 {MAX_FETCH_RECORDS} を超えました"
        ))
    } else {
        Ok(())
    }
}

pub fn push_record(out: &mut Vec<Record>, rec: Record, context: &str) -> Result<()> {
    if out.len() >= MAX_FETCH_RECORDS {
        return Err(anyhow!(
            "{context}: 取得件数が上限 {MAX_FETCH_RECORDS} に達しました"
        ));
    }
    out.push(rec);
    Ok(())
}

pub(crate) fn static_field_names(names: &[&str]) -> Result<Vec<String>> {
    Ok(names.iter().map(|s| (*s).to_string()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_field_names_returns_copy() {
        assert_eq!(
            static_field_names(&["a", "b"]).unwrap(),
            vec!["a".to_string(), "b".to_string()]
        );
    }
}

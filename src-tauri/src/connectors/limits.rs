//! connector 共通の HTTP タイムアウト・ページサイズ・取得件数上限。

use anyhow::{anyhow, Result};
use std::io::Read;
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
/// 公開データの受信・ZIP内の単一CSVをメモリーへ読む上限。
pub const MAX_DATA_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_ARCHIVE_ENTRIES: usize = 4096;
/// 郵便番号など10万行超の正規データを許容しつつ、無制限の行保持を防ぐ。
pub const MAX_PUBLIC_CSV_ROWS: usize = 1_000_000;

pub(crate) fn read_limited(
    reader: impl std::io::Read,
    maximum: u64,
    context: &str,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::io::Read::take(reader, maximum.saturating_add(1)).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err(anyhow!(
            "{context}: 読取り容量の上限を超えました。対象を分割してください"
        ));
    }
    Ok(bytes)
}

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
        // 応答本文やURLには資格情報が含まれ得る。エラー・ログへ転送しない。
        ureq::Error::Status(code, _) => anyhow!("HTTP {method} {code}"),
        ureq::Error::Transport(error) => {
            anyhow!("HTTP {method} への接続に失敗: {:?}", error.kind())
        }
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
    fn response_body_is_never_in_error_messages() {
        let response = ureq::Response::new(403, "Forbidden", "private-response-marker").unwrap();
        let message = map_http_error("GET", ureq::Error::Status(403, response)).to_string();
        assert_eq!(message, "HTTP GET 403");
        assert!(!message.contains("private-response-marker"));
    }

    #[test]
    fn byte_limit_checks_actual_read_and_allows_exact_boundary() {
        assert_eq!(read_limited(&b"1234"[..], 4, "fixture").unwrap(), b"1234");
        assert!(read_limited(&b"12345"[..], 4, "fixture").is_err());
    }

    #[test]
    fn static_field_names_returns_copy() {
        assert_eq!(
            static_field_names(&["a", "b"]).unwrap(),
            vec!["a".to_string(), "b".to_string()]
        );
    }
}

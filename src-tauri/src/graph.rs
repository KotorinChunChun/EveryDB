//! Microsoft Graph 認証・取得基盤（要件 M5）。
//! クライアントクレデンシャルフローでトークンを取得し、`@odata.nextLink` に従って全ページ収集する。

use anyhow::{anyhow, Result};
use serde_json::Value;

pub struct GraphClient {
    token: String,
}

impl GraphClient {
    /// クライアントクレデンシャルフローでアクセストークンを取得する。
    pub fn new(tenant_id: &str, client_id: &str, client_secret: &str) -> Result<Self> {
        let url = format!("https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/token");
        let resp = ureq::post(&url).send_form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("scope", "https://graph.microsoft.com/.default"),
            ("grant_type", "client_credentials"),
        ]);
        let value: Value = match resp {
            Ok(r) => r.into_json()?,
            Err(ureq::Error::Status(code, r)) => {
                let msg = r.into_string().unwrap_or_default();
                return Err(anyhow!("Graph トークン取得失敗 {code}: {msg}"));
            }
            Err(e) => return Err(anyhow!("認証エンドポイントへの接続に失敗: {e}")),
        };
        let token = value
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("access_token が返りませんでした"))?
            .to_string();
        Ok(Self { token })
    }

    /// 単一 GET（JSON を返す）。
    pub fn get(&self, url: &str) -> Result<Value> {
        let resp = ureq::get(url)
            .set("Authorization", &format!("Bearer {}", self.token))
            .call();
        match resp {
            Ok(r) => Ok(r.into_json()?),
            Err(ureq::Error::Status(code, r)) => {
                Err(anyhow!("Graph GET {code}: {}", r.into_string().unwrap_or_default()))
            }
            Err(e) => Err(anyhow!("Graph への接続に失敗: {e}")),
        }
    }

    /// `value` 配列を `@odata.nextLink` に従って全ページ収集する。
    pub fn get_all(&self, first_url: &str) -> Result<Vec<Value>> {
        let mut out = Vec::new();
        let mut next = Some(first_url.to_string());
        while let Some(url) = next {
            let page = self.get(&url)?;
            if let Some(arr) = page.get("value").and_then(|v| v.as_array()) {
                out.extend(arr.iter().cloned());
            }
            next = page.get("@odata.nextLink").and_then(|v| v.as_str()).map(String::from);
        }
        Ok(out)
    }
}

/// "https://host/sites/xxx" を (host, "/sites/xxx") に分解する。
pub fn parse_site_url(u: &str) -> Result<(String, String)> {
    let s = u.trim().trim_end_matches('/');
    let s = s.strip_prefix("https://").or_else(|| s.strip_prefix("http://")).unwrap_or(s);
    let idx = s.find('/').ok_or_else(|| anyhow!("不正なサイト URL: {u}"))?;
    Ok((s[..idx].to_string(), s[idx..].to_string()))
}

/// サイト URL から Graph のサイト ID（複合 ID）を解決する。
pub fn site_id(client: &GraphClient, site_url: &str) -> Result<String> {
    let (host, path) = parse_site_url(site_url)?;
    let v = client.get(&format!("https://graph.microsoft.com/v1.0/sites/{host}:{path}"))?;
    v.get("id")
        .and_then(|x| x.as_str())
        .map(String::from)
        .ok_or_else(|| anyhow!("サイト ID を解決できません: {site_url}"))
}

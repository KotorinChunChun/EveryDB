//! Microsoft Graph 認証・取得基盤（要件 M5）。
//! クライアントクレデンシャルフローでトークンを取得し、`@odata.nextLink` に従って全ページ収集する。

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::connectors::limits::{self, GRAPH_PAGE_TOP};

pub struct GraphClient {
    token: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct GraphAccountIdentity {
    pub id: String,
    pub display_id: String,
}

impl GraphClient {
    /// 既に取得済みのアクセストークンでクライアントを構築する（委任 OAuth 用）。
    pub fn new_with_token(access_token: impl Into<String>) -> Self {
        Self {
            token: access_token.into(),
        }
    }

    /// クライアントクレデンシャルフローでアクセストークンを取得する。
    pub fn new(tenant_id: &str, client_id: &str, client_secret: &str) -> Result<Self> {
        let url = format!("https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/token");
        let resp = limits::http_post_form(
            &url,
            &[
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("scope", "https://graph.microsoft.com/.default"),
                ("grant_type", "client_credentials"),
            ],
        );
        let value: Value = match resp {
            Ok(r) => r.into_json()?,
            Err(e) => return Err(anyhow!("Graph トークン取得失敗: {e}")),
        };
        let token = value
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("access_token が返りませんでした"))?
            .to_string();
        Ok(Self { token })
    }

    /// 取得済みトークンを同一プロセス内の認証処理で参照する。
    /// IPC へは返さず、JWT の実効権限クレーム確認などにだけ使用する。
    pub(crate) fn access_token(&self) -> &str {
        &self.token
    }

    /// 単一 GET（JSON を返す）。
    pub fn get(&self, url: &str) -> Result<Value> {
        let resp = limits::http_get_bearer(url, &self.token);
        match resp {
            Ok(r) => Ok(r.into_json()?),
            Err(e) => Err(anyhow!("Graph GET: {e}")),
        }
    }

    /// `value` 配列を `@odata.nextLink` に従って全ページ収集する。
    pub fn get_all(&self, first_url: &str) -> Result<Vec<Value>> {
        let mut out = Vec::new();
        let mut next = Some(first_url.to_string());
        while let Some(url) = next {
            let page = self.get(&url)?;
            if let Some(arr) = page.get("value").and_then(|v| v.as_array()) {
                for item in arr {
                    limits::check_fetch_limit(out.len() + 1, "Microsoft Graph")?;
                    out.push(item.clone());
                }
            }
            next = page
                .get("@odata.nextLink")
                .and_then(|v| v.as_str())
                .map(String::from);
        }
        Ok(out)
    }

    /// Graph 一覧 URL に `$top` が無ければ付与する。
    pub fn with_page_top(url: &str) -> String {
        if url.contains("$top=") {
            url.to_string()
        } else if url.contains('?') {
            format!("{url}&$top={GRAPH_PAGE_TOP}")
        } else {
            format!("{url}?$top={GRAPH_PAGE_TOP}")
        }
    }
}

fn account_identity_from_value(value: &Value) -> Result<GraphAccountIdentity> {
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("Microsoft アカウントIDが返りませんでした"))?
        .to_string();
    let display_id = ["mail", "userPrincipalName", "displayName"]
        .iter()
        .find_map(|key| {
            value
                .get(*key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
        .unwrap_or(&id)
        .to_string();
    Ok(GraphAccountIdentity { id, display_id })
}

/// 委任アクセストークンでサインインした本人の非秘密識別情報だけを取得する。
pub fn signed_in_account(access_token: &str) -> Result<GraphAccountIdentity> {
    let value = GraphClient::new_with_token(access_token)
        .get("https://graph.microsoft.com/v1.0/me?$select=id,displayName,mail,userPrincipalName")?;
    account_identity_from_value(&value)
}

/// "https://host/sites/xxx" を (host, "/sites/xxx") に分解する。
pub fn parse_site_url(u: &str) -> Result<(String, String)> {
    let s = u.trim().trim_end_matches('/');
    let s = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s);
    let idx = s
        .find('/')
        .ok_or_else(|| anyhow!("不正なサイト URL: {u}"))?;
    Ok((s[..idx].to_string(), s[idx..].to_string()))
}

/// サイト URL から Graph のサイト ID（複数 ID）を解決する。
pub fn site_id(client: &GraphClient, site_url: &str) -> Result<String> {
    let (host, path) = parse_site_url(site_url)?;
    let v = client.get(&format!(
        "https://graph.microsoft.com/v1.0/sites/{host}:{path}"
    ))?;
    v.get("id")
        .and_then(|x| x.as_str())
        .map(String::from)
        .ok_or_else(|| anyhow!("サイト ID を解決できません: {site_url}"))
}

#[cfg(test)]
mod account_identity_tests {
    use super::*;

    #[test]
    fn prefers_mail_and_keeps_the_stable_microsoft_id() {
        let identity = account_identity_from_value(&serde_json::json!({
            "id": "account-guid",
            "displayName": "Every DB",
            "mail": "everydb@example.com",
            "userPrincipalName": "everydb@example.onmicrosoft.com"
        }))
        .unwrap();
        assert_eq!(identity.id, "account-guid");
        assert_eq!(identity.display_id, "everydb@example.com");
    }

    #[test]
    fn falls_back_to_upn_then_account_id() {
        let with_upn = account_identity_from_value(&serde_json::json!({
            "id": "account-guid",
            "userPrincipalName": "user@example.com"
        }))
        .unwrap();
        assert_eq!(with_upn.display_id, "user@example.com");

        let id_only = account_identity_from_value(&serde_json::json!({
            "id": "account-guid"
        }))
        .unwrap();
        assert_eq!(id_only.display_id, "account-guid");
    }
}

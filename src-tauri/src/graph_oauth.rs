//! Microsoft Graph 委任 OAuth 2.0（Authorization Code + PKCE + ループバック redirect）。
//!
//! パブリッククライアント想定のためトークン交換に client_secret は使わない。
//! Redirect URI は Box と同様に固定ポート（手順書・Entra 登録と完全一致させること）。

use anyhow::{anyhow, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

const REDIRECT_PORT: u16 = 58914;
const REDIRECT_PATH: &str = "/graph-callback";
const AUTHORIZE_TIMEOUT: Duration = Duration::from_secs(180);

/// Entra アプリ登録・手順書と一致させる固定 Redirect URI。
pub const REDIRECT_URI: &str = "http://localhost:58914/graph-callback";

/// Graph 委任認可で要求するスコープ（SharePoint Phase 1）。
pub const DELEGATED_SCOPES: &str =
    "https://graph.microsoft.com/Sites.Read.All offline_access User.Read";

/// フロントが「ユーザー取消」と失敗を区別するための固定メッセージ。
pub const CANCELLED_MESSAGE: &str = "Microsoft の認可をキャンセルしました";

fn random_hex(bytes_len: usize) -> Result<String> {
    let mut bytes = vec![0_u8; bytes_len];
    getrandom::fill(&mut bytes)
        .map_err(|error| anyhow!("OAuth 乱数生成に失敗しました: {error}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn random_state() -> Result<String> {
    random_hex(32)
}

fn pkce_verifier() -> Result<String> {
    // RFC 7636: 43–128 文字の base64url。32 bytes → 43 chars (no pad).
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|error| anyhow!("PKCE verifier の乱数生成に失敗しました: {error}"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn pkce_challenge_s256(verifier: &str) -> String {
    let digest = hmac_sha256::Hash::hash(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn bind_loopback() -> Result<TcpListener> {
    TcpListener::bind(("127.0.0.1", REDIRECT_PORT)).map_err(|e| {
        anyhow!(
            "ローカルポート {REDIRECT_PORT} を確保できませんでした（他のアプリが使用中の可能性があります）: {e}"
        )
    })
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn urldecode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hex = &s[i + 1..i + 3];
                if let Ok(v) = u8::from_str_radix(hex, 16) {
                    out.push(v);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn parse_query(query: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let mut it = pair.splitn(2, '=');
        let k = it.next().unwrap_or("");
        let v = it.next().unwrap_or("");
        out.insert(urldecode(k), urldecode(v));
    }
    out
}

fn authorize_url(
    tenant_id: &str,
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    code_challenge: &str,
) -> String {
    format!(
        "https://login.microsoftonline.com/{}/oauth2/v2.0/authorize?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        urlencode(tenant_id),
        urlencode(client_id),
        urlencode(redirect_uri),
        urlencode(DELEGATED_SCOPES),
        urlencode(state),
        urlencode(code_challenge),
    )
}

fn wait_for_callback(
    listener: TcpListener,
    expected_state: &str,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<String> {
    wait_for_callback_with_timeout(listener, expected_state, cancel, AUTHORIZE_TIMEOUT)
}

fn wait_for_callback_with_timeout(
    listener: TcpListener,
    expected_state: &str,
    cancel: &std::sync::atomic::AtomicBool,
    timeout: Duration,
) -> Result<String> {
    use std::sync::atomic::Ordering;
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + timeout;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                if let Some(result) = handle_callback_connection(stream, expected_state) {
                    return result;
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if cancel.load(Ordering::Relaxed) {
                    return Err(anyhow!("{CANCELLED_MESSAGE}"));
                }
                if Instant::now() >= deadline {
                    return Err(anyhow!(
                        "認可がタイムアウトしました。3分以内にブラウザでの操作を完了してください"
                    ));
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(e.into()),
        }
    }
}

fn handle_callback_connection(
    mut stream: TcpStream,
    expected_state: &str,
) -> Option<Result<String>> {
    stream.set_nonblocking(false).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut buf = [0u8; 8192];
    let n = stream.read(&mut buf).ok()?;
    if n == 0 {
        return None;
    }
    let req = String::from_utf8_lossy(&buf[..n]);
    let first_line = req.lines().next().unwrap_or("");
    let path_and_query = first_line.split_whitespace().nth(1).unwrap_or("");
    if !path_and_query.starts_with(REDIRECT_PATH) {
        return None;
    }
    let query = path_and_query.split('?').nth(1).unwrap_or("");
    let params = parse_query(query);
    if params.is_empty() {
        return None;
    }

    let result = match (params.get("code"), params.get("state")) {
        (Some(code), Some(state)) if state == expected_state => Ok(code.clone()),
        (Some(_), Some(_)) => Err(anyhow!(
            "認可応答の state が一致しません（別セッションの応答の可能性があります）"
        )),
        _ => {
            let err = params
                .get("error_description")
                .or_else(|| params.get("error"))
                .cloned()
                .unwrap_or_else(|| "不明なエラー".to_string());
            Err(anyhow!(
                "Microsoft からの応答にエラーが含まれています: {err}"
            ))
        }
    };

    let body = if result.is_ok() {
        "<html><body style=\"font-family:sans-serif\"><p>Microsoft の認可が完了しました。このタブは閉じて EveryDB に戻ってください。</p></body></html>"
    } else {
        "<html><body style=\"font-family:sans-serif\"><p>Microsoft の認可に失敗しました。EveryDB に戻ってやり直してください。</p></body></html>"
    };
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
    Some(result)
}

fn token_request(tenant_id: &str, form: &[(&str, &str)]) -> Result<(String, String)> {
    let url = format!("https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/token");
    let resp = ureq::post(&url).send_form(form);
    let body: Value = match resp {
        Ok(r) => r.into_json()?,
        Err(ureq::Error::Status(code, r)) => {
            return Err(anyhow!(
                "Microsoft トークン取得失敗 {code}: {}",
                r.into_string().unwrap_or_default()
            ));
        }
        Err(e) => return Err(anyhow!("Microsoft 認証エンドポイントへの接続に失敗: {e}")),
    };
    let access_token = body
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("access_token が返りませんでした"))?
        .to_string();
    let refresh_token = body
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    Ok((access_token, refresh_token))
}

/// ブラウザ認可 → ローカルでコード受信 → PKCE 付きトークン交換。
/// 戻り値は (access_token, refresh_token)。
pub fn authorize_interactive(
    app: &tauri::AppHandle,
    tenant_id: &str,
    client_id: &str,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<(String, String)> {
    use tauri_plugin_opener::OpenerExt;
    let listener = bind_loopback()?;
    let state = random_state()?;
    let verifier = pkce_verifier()?;
    let challenge = pkce_challenge_s256(&verifier);
    let url = authorize_url(tenant_id, client_id, REDIRECT_URI, &state, &challenge);
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| anyhow!("ブラウザを開けませんでした: {e}"))?;
    let code = wait_for_callback(listener, &state, cancel)?;
    token_request(
        tenant_id,
        &[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("client_id", client_id),
            ("redirect_uri", REDIRECT_URI),
            ("code_verifier", &verifier),
            ("scope", DELEGATED_SCOPES),
        ],
    )
}

/// refresh_token から access_token を再取得する。新しい refresh_token が返った場合は呼び出し側で保存すること。
pub fn refresh(
    tenant_id: &str,
    client_id: &str,
    refresh_token: &str,
) -> Result<(String, String)> {
    token_request(
        tenant_id,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
            ("scope", DELEGATED_SCOPES),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_state_is_url_safe_and_unpredictable() {
        let first = random_state().unwrap();
        let second = random_state().unwrap();
        assert_eq!(first.len(), 64);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_ne!(first, second);
    }

    #[test]
    fn pkce_verifier_and_challenge_are_s256_base64url() {
        let verifier = pkce_verifier().unwrap();
        assert!(verifier.len() >= 43);
        assert!(!verifier.contains('+') && !verifier.contains('/') && !verifier.contains('='));
        let challenge = pkce_challenge_s256(&verifier);
        assert!(!challenge.is_empty());
        assert!(!challenge.contains('+') && !challenge.contains('/') && !challenge.contains('='));
        assert_ne!(verifier, challenge);
    }

    #[test]
    fn authorize_url_includes_pkce_and_scopes() {
        let url = authorize_url(
            "tenant-1",
            "cid 1",
            "http://localhost:58914/graph-callback",
            "st ate",
            "challenge+",
        );
        assert!(url.starts_with(
            "https://login.microsoftonline.com/tenant-1/oauth2/v2.0/authorize?"
        ));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("client_id=cid%201"));
        assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A58914%2Fgraph-callback"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("code_challenge=challenge%2B"));
        assert!(url.contains("state=st%20ate"));
        assert!(url.contains("Sites.Read.All") || url.contains("Sites.Read.All".replace('.', "%2E").as_str()) || url.contains("graph.microsoft.com"));
        assert!(url.contains("offline_access") || url.contains("offline_access"));
    }

    #[test]
    fn redirect_uri_matches_the_fixed_port() {
        assert_eq!(
            REDIRECT_URI,
            format!("http://localhost:{REDIRECT_PORT}{REDIRECT_PATH}")
        );
    }

    fn callback_result(request_target: &str, expected_state: &str) -> Result<String> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let target = request_target.to_string();
        let sender = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            let request =
                format!("GET {target} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
            stream.write_all(request.as_bytes()).unwrap();
        });
        let (stream, _) = listener.accept().unwrap();
        let result = handle_callback_connection(stream, expected_state)
            .expect("callback must be recognized");
        sender.join().unwrap();
        result
    }

    #[test]
    fn callback_requires_matching_state_and_surfaces_provider_errors() {
        assert_eq!(
            callback_result("/graph-callback?code=issued&state=expected", "expected").unwrap(),
            "issued"
        );
        assert!(callback_result("/graph-callback?code=issued&state=other", "expected")
            .unwrap_err()
            .to_string()
            .contains("state が一致"));
        assert!(callback_result("/graph-callback?error=access_denied", "expected")
            .unwrap_err()
            .to_string()
            .contains("access_denied"));
    }

    #[test]
    fn callback_wait_honors_cancel_and_timeout_without_external_services() {
        use std::sync::atomic::AtomicBool;

        let cancelled_listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let cancelled = AtomicBool::new(true);
        assert!(wait_for_callback_with_timeout(
            cancelled_listener,
            "state",
            &cancelled,
            Duration::ZERO
        )
        .unwrap_err()
        .to_string()
        .contains(CANCELLED_MESSAGE));

        let timed_out_listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let active = AtomicBool::new(false);
        assert!(wait_for_callback_with_timeout(
            timed_out_listener,
            "state",
            &active,
            Duration::ZERO
        )
        .unwrap_err()
        .to_string()
        .contains("タイムアウト"));
    }
}

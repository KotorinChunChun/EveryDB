//! Box 認証（OAuth 2.0 ブラウザ認可 / Client Credentials Grant の両対応）。
//!
//! 個人（無料）プランの Box アカウントは Client Credentials Grant（サーバー認証・
//! box_subject_type=enterprise、Enterprise 管理者の承認が要る Enterprise 専用機能）を
//! 利用できないため、通常の OAuth 2.0 ブラウザ認可でユーザー自身のアカウントとして認証する
//! 経路をこのモジュールで提供する。Enterprise プランで CCG を使いたい場合向けに、
//! 従来どおりの `client_credentials_token()` も残してある（§7-18: 両方を選べるようにしてほしい）。
//!
//! redirect_uri は固定ポートのループバックアドレスを使う。実機で試したところ、Box は
//! ドキュメント上「ループバックならポート番号を問わずマッチする」とされているにもかかわらず
//! 実際には `redirect_uri_mismatch` になるケースがあったため、動的ポートには頼らず、
//! Developer Console に登録してもらう URI（ポート込みの完全一致）を固定にしている。

use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

const REDIRECT_PORT: u16 = 58913;
const REDIRECT_PATH: &str = "/box-callback";
const AUTHORIZE_TIMEOUT: Duration = Duration::from_secs(180);

/// 登録してもらう redirect_uri（トークン管理画面のヘルプ文言・手順書と一致させること）。
pub const REDIRECT_URI: &str = "http://localhost:58913/box-callback";

/// OAuth 2.0 の CSRF state。OSの暗号学的乱数から256-bitを生成し、URLに安全な16進文字列へする。
fn random_state() -> Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|error| anyhow!("OAuth stateの乱数生成に失敗しました: {error}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// 固定ポートのループバックへバインドする。使用中の場合は分かりやすいエラーを返す。
fn bind_loopback() -> Result<TcpListener> {
    TcpListener::bind(("127.0.0.1", REDIRECT_PORT)).map_err(|e| {
        anyhow!("ローカルポート {REDIRECT_PORT} を確保できませんでした（他のアプリが使用中の可能性があります）: {e}")
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
            b'%' if i + 2 < bytes.len() => {
                if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(b);
                    i += 3;
                    continue;
                }
                out.push(bytes[i]);
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

fn parse_query(query: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let mut it = pair.splitn(2, '=');
        let k = it.next().unwrap_or("");
        let v = it.next().unwrap_or("");
        out.insert(urldecode(k), urldecode(v));
    }
    out
}

fn authorize_url(client_id: &str, redirect_uri: &str, state: &str) -> String {
    format!(
        "https://account.box.com/api/oauth2/authorize?response_type=code&client_id={}&redirect_uri={}&state={}",
        urlencode(client_id),
        urlencode(redirect_uri),
        urlencode(state),
    )
}

/// キャンセルされたことを表す固定のエラーメッセージ。呼び出し側（フロント）はこの文字列で
/// 「ユーザー自身の取消操作」と「本当の失敗」を区別し、前者ではエラー表示をしない。
pub const CANCELLED_MESSAGE: &str = "Box の認可をキャンセルしました";

/// リダイレクトを1件だけ受け付けてクエリを取り出す（ブロッキング）。
/// 呼び出し側（Tauri コマンド）は必ず spawn_blocking 等で別スレッドに逃がすこと
/// （メインスレッドで待つとアプリがフリーズする。既存の Windows Hello 実装と同じ注意点）。
/// `cancel` が true になったら（要件: 別プロファイルで誤って開いた等の理由での取消。§7-19）、
/// タイムアウトを待たずに即座に諦める。
fn wait_for_callback(
    listener: TcpListener,
    expected_state: &str,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<String> {
    use std::sync::atomic::Ordering;
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + AUTHORIZE_TIMEOUT;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                // 本命のコールバックでない接続（ブラウザの投機的プリコネクト、favicon 要求など）は
                // 無視して待機を続ける。ここで打ち切ると認可が成立しなくなる。
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

/// 受理した1接続を処理する。認可コールバックとして扱えない接続（データを送ってこない
/// プリコネクト、`/favicon.ico` 等）では `None` を返し、呼び出し側に待機を続けさせる。
fn handle_callback_connection(
    mut stream: TcpStream,
    expected_state: &str,
) -> Option<Result<String>> {
    stream.set_nonblocking(false).ok()?;
    // データを送ってこない接続で無期限にブロックしないよう読み取り制限時間を設ける
    // （ここで固まると取消ボタンも効かなくなる）。
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
            Err(anyhow!("Box からの応答にエラーが含まれています: {err}"))
        }
    };

    let body = if result.is_ok() {
        "<html><body style=\"font-family:sans-serif\"><p>Box の認可が完了しました。このタブは閉じて EveryDB に戻ってください。</p></body></html>"
    } else {
        "<html><body style=\"font-family:sans-serif\"><p>Box の認可に失敗しました。EveryDB に戻ってやり直してください。</p></body></html>"
    };
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
    Some(result)
}

fn token_request(form: &[(&str, &str)]) -> Result<(String, String)> {
    let resp = ureq::post("https://api.box.com/oauth2/token").send_form(form);
    let body: Value = match resp {
        Ok(r) => r.into_json()?,
        Err(ureq::Error::Status(code, r)) => {
            return Err(anyhow!(
                "Box トークン取得失敗 {code}: {}",
                r.into_string().unwrap_or_default()
            ));
        }
        Err(e) => return Err(anyhow!("Box 認証エンドポイントへの接続に失敗: {e}")),
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

/// ブラウザでの認可 → ローカルでコード受信 → トークン交換、を一括して行う（OAuth 2.0）。
/// 戻り値は (access_token, refresh_token)。`cancel` が true になったら待機を打ち切る
/// （要件: 間違えて別のプロファイルで開いたときのための取消ボタン。§7-19）。
pub fn authorize_interactive(
    app: &tauri::AppHandle,
    client_id: &str,
    client_secret: &str,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<(String, String)> {
    use tauri_plugin_opener::OpenerExt;
    let listener = bind_loopback()?;
    let state = random_state()?;
    let url = authorize_url(client_id, REDIRECT_URI, &state);
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| anyhow!("ブラウザを開けませんでした: {e}"))?;
    let code = wait_for_callback(listener, &state, cancel)?;
    token_request(&[
        ("grant_type", "authorization_code"),
        ("code", &code),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("redirect_uri", REDIRECT_URI),
    ])
}

/// リフレッシュトークンからアクセストークンを再取得する（OAuth 2.0）。Box はリフレッシュの
/// たびに refresh_token 自体もローテーションする（旧トークンは失効する）ため、戻り値の
/// 新しい refresh_token を呼び出し側で必ず保存し直すこと。
pub fn refresh(
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<(String, String)> {
    token_request(&[
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", client_id),
        ("client_secret", client_secret),
    ])
}

/// Client Credentials Grant（法人/Enterprise プラン専用。Enterprise 管理者の承認が必要）で
/// サービスアカウントのアクセストークンを取得する（§7-18: 法人用の選択肢として残す）。
/// CCG のアクセストークンは毎回その場で新規発行されるだけで、refresh_token のような
/// ローテーション管理は不要。
pub fn client_credentials_token(
    client_id: &str,
    client_secret: &str,
    enterprise_id: &str,
) -> Result<String> {
    let (access_token, _refresh_token) = token_request(&[
        ("grant_type", "client_credentials"),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("box_subject_type", "enterprise"),
        ("box_subject_id", enterprise_id),
    ])?;
    Ok(access_token)
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
    fn authorize_url_encodes_and_includes_required_params() {
        let url = authorize_url("cid 1", "http://localhost:12345/box-callback", "st ate");
        assert!(url.starts_with("https://account.box.com/api/oauth2/authorize?"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("client_id=cid%201"));
        assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A12345%2Fbox-callback"));
        assert!(url.contains("state=st%20ate"));
    }

    #[test]
    fn parse_query_decodes_percent_and_plus() {
        let m = parse_query("code=abc%2Bdef&state=foo+bar");
        assert_eq!(m.get("code").unwrap(), "abc+def");
        assert_eq!(m.get("state").unwrap(), "foo bar");
    }

    #[test]
    fn redirect_uri_matches_the_fixed_port() {
        assert_eq!(
            REDIRECT_URI,
            format!("http://localhost:{REDIRECT_PORT}{REDIRECT_PATH}")
        );
    }
}

//! KotorinToysのユーザー専用管理契約2を実装する。
mod pipe;

use anyhow::{ensure, Context};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use tauri::AppHandle;

const TOOL_ID: &str = "every-db";
const MAX_REQUEST_BYTES: usize = 8192;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

pub struct Management {
    ready: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
}

impl Management {
    /// Tauriのsetup中に呼び、管理パイプの作成完了後に戻る。
    pub fn start(app: AppHandle) -> anyhow::Result<Self> {
        let ready = Arc::new(AtomicBool::new(false));
        let shutdown = Arc::new(AtomicBool::new(false));
        let server_ready = ready.clone();
        let server_shutdown = shutdown.clone();
        let (started_tx, started_rx) = mpsc::sync_channel(1);
        let launch_origin = launch_origin();

        std::thread::Builder::new()
            .name("everydb-management-pipe".into())
            .spawn(move || {
                let server = match pipe::ManagementPipe::create(TOOL_ID) {
                    Ok(server) => {
                        let _ = started_tx.send(Ok(()));
                        server
                    }
                    Err(error) => {
                        let _ = started_tx.send(Err(error.to_string()));
                        return;
                    }
                };

                while !server_shutdown.load(Ordering::Acquire) {
                    if server.accept(Duration::from_millis(100)).is_err() {
                        continue;
                    }
                    let deadline = Instant::now() + REQUEST_TIMEOUT;
                    let stop =
                        handle_request(&server, &app, &server_ready, &launch_origin, deadline);
                    server.wait_for_client_close(deadline);
                    server.disconnect();
                    if stop {
                        app.exit(0);
                        break;
                    }
                }
            })
            .context("管理窓口のスレッドを起動できません")?;

        match started_rx.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(())) => Ok(Self { ready, shutdown }),
            Ok(Err(detail)) => {
                shutdown.store(true, Ordering::Release);
                anyhow::bail!("管理窓口を作成できません: {detail}")
            }
            Err(error) => {
                shutdown.store(true, Ordering::Release);
                Err(error).context("管理窓口の起動が時間切れです")
            }
        }
    }

    /// Tauriのイベントループが動き始めてから準備完了を返す。
    pub fn set_ready(&self) {
        self.ready.store(true, Ordering::Release);
    }
}

impl Drop for Management {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
    }
}

fn launch_origin() -> String {
    let args = std::env::args().collect::<Vec<_>>();
    let Some(index) = args.iter().position(|arg| arg == "--launch-origin") else {
        return "standalone".into();
    };
    match args.get(index + 1).map(String::as_str) {
        Some(origin @ ("via_kotorintoys" | "direct_task" | "host_manual")) => origin.into(),
        _ => "standalone".into(),
    }
}

fn handle_request(
    server: &pipe::ManagementPipe,
    app: &AppHandle,
    ready: &AtomicBool,
    launch_origin: &str,
    deadline: Instant,
) -> bool {
    let mut command = String::new();
    let mut request_id = String::new();
    let mut expected_pid = None;
    let mut result = "completed";
    let mut detail = String::new();
    let mut stop = false;

    let parsed = server
        .read_line(deadline)
        .ok()
        .and_then(|line| parse_request(&line).ok());
    if let Some((parsed_command, parsed_request_id, parsed_pid)) = parsed {
        command = parsed_command;
        request_id = parsed_request_id;
        expected_pid = parsed_pid;
    } else {
        result = "failed";
        detail = "管理要求を読み取れませんでした。".into();
    }

    if !server.same_session().unwrap_or(false) {
        result = "refused";
        detail = "別の対話セッションからの要求を拒否しました。".into();
    }

    let is_ready = ready.load(Ordering::Acquire);
    if result == "completed" {
        match command.as_str() {
            "status" => {}
            "show" | "stop" if !is_ready => {
                result = "refused";
                detail = "起動処理中です。状態を確認してから再試行してください。".into();
            }
            "show" | "stop" if expected_pid != Some(std::process::id()) => {
                result = "refused";
                detail = "対象PIDが一致しません。".into();
            }
            "show" => {
                if show_on_main_thread(app, deadline) {
                    result = "completed";
                } else {
                    result = "failed";
                    detail = "画面表示を完了できませんでした。".into();
                }
            }
            "stop" => {
                result = "accepted";
                stop = true;
            }
            _ => {
                result = "unsupported";
                detail = "未対応の操作です。".into();
            }
        }
    }

    let response = json!({
        "protocolVersion": 2,
        "requestId": request_id,
        "pid": std::process::id(),
        "ready": is_ready,
        "version": env!("CARGO_PKG_VERSION"),
        "launchOrigin": launch_origin,
        "capabilities": ["status", "show", "stop"],
        "result": result,
        "detail": detail,
    });
    let response_written = serde_json::to_string(&response)
        .ok()
        .is_some_and(|line| server.write_line(&line, deadline).is_ok());
    stop && response_written
}

fn parse_request(line: &str) -> anyhow::Result<(String, String, Option<u32>)> {
    if line == "status" {
        return Ok(("status".into(), String::new(), None));
    }
    ensure!(line.len() <= MAX_REQUEST_BYTES, "管理要求が長すぎます");
    let value: Value = serde_json::from_str(line).context("管理要求がJSONではありません")?;
    let command = value
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let request_id = value
        .get("requestId")
        .and_then(Value::as_str)
        .filter(|id| id.len() <= 128)
        .unwrap_or_default()
        .to_owned();
    let expected_pid = value
        .get("expectedPid")
        .and_then(Value::as_u64)
        .and_then(|pid| u32::try_from(pid).ok());
    Ok((command, request_id, expected_pid))
}

fn show_on_main_thread(app: &AppHandle, deadline: Instant) -> bool {
    let (shown_tx, shown_rx) = mpsc::sync_channel(1);
    let app_for_ui = app.clone();
    if app
        .run_on_main_thread(move || {
            let shown = Instant::now() < deadline && crate::show_main_window(&app_for_ui);
            let _ = shown_tx.send(shown);
        })
        .is_err()
    {
        return false;
    }
    shown_rx
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::parse_request;

    #[test]
    fn parses_status_and_pid_bound_commands() {
        assert_eq!(
            parse_request("status").unwrap(),
            ("status".into(), String::new(), None)
        );
        assert_eq!(
            parse_request(r#"{"command":"show","expectedPid":42,"requestId":"abc"}"#).unwrap(),
            ("show".into(), "abc".into(), Some(42))
        );
    }

    #[test]
    fn rejects_malformed_or_oversized_requests() {
        assert!(parse_request("{").is_err());
        assert!(parse_request(&format!(
            "{{\"command\":\"status\",\"padding\":\"{}\"}}",
            "x".repeat(8192)
        ))
        .is_err());
    }
}

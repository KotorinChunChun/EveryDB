use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tauri::{AppHandle, Manager, State};

use crate::app_state::AppState;

fn err_to_string<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}
// ---- 秘密情報の閲覧ゲート（Windows Hello による本人確認）----
// 以前はアプリ独自の「マスターパスワード」を DB に保存して使っていたが、DPAPI による暗号化が
// 既に Windows ユーザーに紐づいているため、その上にアプリ独自パスワードを持つ必然性が無く、
// OS 標準の本人確認（Windows Hello: 顔 / 指紋 / PIN）に置き換えた。本ツールの DB は
// パスワード（ハッシュ含む）を一切保持しない。

// winauth_status / winauth_verify は async fn にする。中身が呼ぶ WinRT の IAsyncOperation::get()
// はブロッキング待機のため、素の同期 fn のままだと add_source 等と同じ理由（コメント参照）で
// Tauri のメインスレッドを塞いでしまう。実際に「再表示ボタンを押すとアプリごとフリーズし、
// Windows Hello の画面も出ない」不具合として発生した（メインスレッドが自分自身の待機で
// 塞がり、ダイアログの表示・完了通知も進まなくなっていたとみられる）。async fn にすることで
// Tauri の非同期ランタイム側のスレッドで実行され、UI スレッドは塞がれない。
#[tauri::command]
pub async fn winauth_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    Ok(
        serde_json::json!({ "available": crate::winauth::is_available(), "unlocked": state.is_unlocked() }),
    )
}

/// Windows Hello の確認ダイアログを出す。直近 10 分以内に確認済みなら再要求しない
/// （`is_unlocked` を先にチェックする点は呼び出し側の役割）。
/// Windows Hello が使えない環境（PIN 未設定・非対応ハード等）では確認をスキップして許可する
/// （DPAPI 自体は引き続き有効なので、リモート攻撃者が横から secrets を読めるようにはならない）。
#[tauri::command]
pub async fn winauth_verify(app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    if !crate::winauth::is_available() {
        state.unlock();
        return Ok(true);
    }
    #[cfg(windows)]
    let hwnd = app
        .get_webview_window("main")
        .ok_or("メインウィンドウが見つかりません")?
        .hwnd()
        .map_err(err_to_string)?;
    #[cfg(not(windows))]
    let hwnd = {
        let _ = &app;
        ()
    };
    let ok = crate::winauth::verify(hwnd, "EveryDB: 保存済みのトークンを表示します")
        .map_err(err_to_string)?;
    if ok {
        state.unlock();
    }
    Ok(ok)
}

// ---- 保存済みトークン（要件: トークン管理画面。過去に設定したトークンの再利用・一括置換）----

#[tauri::command]
pub fn list_credentials(
    state: State<'_, AppState>,
    scope: String,
) -> Result<Vec<crate::credentials::CredentialSummary>, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    crate::credentials::list_credentials(&conn, &scope).map_err(err_to_string)
}

#[tauri::command]
pub fn add_credential(
    state: State<'_, AppState>,
    scope: String,
    label: String,
    fields_json: String,
) -> Result<i64, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    crate::credentials::add_credential(&conn, &scope, &label, &fields_json).map_err(err_to_string)
}

/// 「置換」。値を書き換えると、参照している全ソースへ自動的に反映される（共有参照方式）。
#[tauri::command]
pub fn update_credential(
    state: State<'_, AppState>,
    id: i64,
    label: String,
    fields_json: String,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    crate::credentials::update_credential(&conn, id, &label, &fields_json).map_err(err_to_string)
}

/// 使用中ソースがあれば拒否する（要件: 削除をブロックし使用中ソース一覧を表示）。
#[tauri::command]
pub fn delete_credential(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    crate::credentials::delete_credential(&conn, id)
}

#[tauri::command]
pub fn credential_usage(
    state: State<'_, AppState>,
    id: i64,
) -> Result<Vec<crate::credentials::CredentialUsage>, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let out = crate::credentials::find_usage(&conn, id)
        .map_err(err_to_string)?
        .into_iter()
        .map(
            |(source_id, source_name)| crate::credentials::CredentialUsage {
                source_id,
                source_name,
            },
        )
        .collect();
    Ok(out)
}

/// ピッカー用: ラベルと非秘密フィールドの平文値だけを返す（秘密フィールドの実値は渡さない）。
#[tauri::command]
pub fn credential_public_fields(
    state: State<'_, AppState>,
    id: i64,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let (label, fields) = crate::credentials::public_fields(&conn, id).map_err(err_to_string)?;
    Ok(serde_json::json!({ "label": label, "fields": fields }))
}

/// まだソースとして保存していない新規追加フロー用: 選択済みトークンの秘密フィールド実値を
/// 一度だけ返す（Base一覧・データベース一覧等のライブブラウズに使う）。
#[tauri::command]
pub fn credential_secret_value(state: State<'_, AppState>, id: i64) -> Result<String, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    crate::credentials::resolve_secret_value(&conn, id).map_err(err_to_string)
}

/// Box の OAuth 2.0 認可（要件: 個人プランの Box は Client Credentials Grant を使えないため、
/// 通常のブラウザ認可でユーザー自身のアカウントとして認証する）。
/// システムブラウザを開き、ローカルのループバックで認可応答（コード）を受け取ってトークンに
/// 交換するまでをこのコマンド1回で完結させる。ブロッキング処理（ブラウザでの操作待ち・最大3分）
/// を含むため、必ず spawn_blocking で別スレッドに逃がす（同期のままだと UI が固まる。
/// winauth_verify と同じ理由）。
/// 待機中は `box_oauth_cancel` から取消できるよう、取消フラグを AppState に置いておく
/// （要件: 間違えて別のプロファイルで開いたときのための取消ボタン。§7-19）。
#[tauri::command]
pub async fn box_oauth_authorize(
    app: AppHandle,
    state: State<'_, AppState>,
    client_id: String,
    client_secret: String,
) -> Result<serde_json::Value, String> {
    let client_id = client_id.trim().to_string();
    let client_secret = client_secret.trim().to_string();
    if client_id.is_empty() || client_secret.is_empty() {
        return Err("クライアントIDとクライアントシークレットを先に入力してください".to_string());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut slot = state.box_oauth_cancel.lock().map_err(|_| "lock poisoned")?;
        *slot = Some(cancel.clone());
    }
    let handle = app.clone();
    let cancel_for_task = cancel.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        crate::box_oauth::authorize_interactive(
            &handle,
            &client_id,
            &client_secret,
            &cancel_for_task,
        )
    })
    .await
    .map_err(|e| format!("内部エラー: {e}"))?;
    // 完了したので取消フラグの参照を掃除する（後から取消を押されても何も起きないようにする）。
    if let Ok(mut slot) = state.box_oauth_cancel.lock() {
        *slot = None;
    }
    let (_access_token, refresh_token) = result.map_err(err_to_string)?;
    // リフレッシュトークンが無いと以後の自動更新ができない（保存しても「未認可」のままになる）。
    // 黙って空を返すと成功したように見えてしまうため、ここで明確に失敗させる。
    if refresh_token.is_empty() {
        return Err("Box からリフレッシュトークンが返りませんでした。アプリの認証方式が「ユーザー認証(標準 OAuth 2.0)」になっているか確認してください".to_string());
    }
    Ok(serde_json::json!({ "refresh_token": refresh_token }))
}

/// 実行中の Box OAuth 認可待ちを取り消す（要件: 間違えて別のプロファイルで開いたときのための
/// 取消ボタン。§7-19）。待っている処理が無ければ何もしない（二重押し等でエラーにはしない）。
#[tauri::command]
pub fn box_oauth_cancel(state: State<'_, AppState>) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    let slot = state.box_oauth_cancel.lock().map_err(|_| "lock poisoned")?;
    if let Some(flag) = slot.as_ref() {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

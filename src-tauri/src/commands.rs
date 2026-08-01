pub use crate::app_state::AppState;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use rusqlite::Connection;
use tauri::{AppHandle, Manager, State};

use crate::model::SourceInfo;
use crate::refresh::{optimize_fts, reload_one_for_state};
use crate::{db, secret};

fn err_to_string<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
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

#[tauri::command]
pub fn list_sources(state: State<'_, AppState>) -> Result<Vec<SourceInfo>, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::list_sources(&conn).map_err(err_to_string)
}

// add_source / update_source / add_source_from_sibling / delete_source は非同期コマンドにする。
// これらは初回 fetch（reload_one）でリモート先（Notion/Graph/DB等）への通信を伴い数秒かかることが
// あるが、素の同期 fn のままだと Tauri がメインスレッド上で実行するため、その間ウィンドウ全体が
// 応答しなくなる（要件: 「保存してリロード」でフリーズする不具合）。async fn にするだけで Tauri の
// 非同期ランタイム側のスレッドで実行されるようになり、UI スレッドは塞がれない
// （フロント側の読み込み中スピナー表示 §2-2 が実際に描画されるためにもこれが前提になる）。
#[tauri::command]
pub async fn add_source(
    state: State<'_, AppState>,
    type_: String,
    name: String,
    config_json: String,
) -> Result<i64, String> {
    let id = {
        let conn = state.db.lock().map_err(|_| "lock poisoned")?;
        // 秘密フィールド（Notion トークン等）は DPAPI で暗号化してから保存する（要件 §4-3-bis）。
        let config_json = secret::protect_config(&type_, &config_json).map_err(err_to_string)?;
        db::insert_source(&conn, &type_, &name, &config_json).map_err(err_to_string)?
    };
    // 追加直後に一度取得しておく。外部I/O中はDBロックを保持しない。
    let _ = reload_one_for_state(&state, id);
    optimize_fts(&state);
    Ok(id)
}

/// 新規テーブルの設定に、兄弟ソースの接続情報（保存されている暗号化済みの値）を補う。
/// 空・プレースホルダーのキーだけが対象。`dpapi:` 付きの値はそのままコピーされ、
/// `protect_config` が二重に暗号化しないため復号可能な状態を保てる。
fn fill_shared_from_sibling(
    config_json: &str,
    sibling_config_json: &str,
    shared_keys: &[String],
) -> Result<String, String> {
    let sibling: serde_json::Value =
        serde_json::from_str(sibling_config_json).map_err(err_to_string)?;
    let mut cfg: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(config_json).map_err(err_to_string)?;
    for k in shared_keys {
        let cur = cfg.get(k).and_then(|v| v.as_str()).unwrap_or_default();
        if cur.is_empty() || cur == secret::SECRET_PLACEHOLDER {
            if let Some(v) = sibling.get(k) {
                cfg.insert(k.clone(), v.clone());
            }
        }
    }
    serde_json::to_string(&cfg).map_err(err_to_string)
}

/// 既存の兄弟ソースから接続情報を引き継いで、同じ接続先配下にテーブルを追加する。
/// 接続設定ダイアログのテーブル一覧から追加するときに使う。
///
/// UI 側の秘密フィールドは未確認時マスク（`••••••••`）されているため、その値で追加すると
/// 壊れたソースができてしまう。ここでは**暗号化されたままの値をコピー**することで、
/// UI が平文の秘密情報を扱わずに済むようにしている。
#[tauri::command]
pub async fn add_source_from_sibling(
    state: State<'_, AppState>,
    sibling_id: i64,
    type_: String,
    name: String,
    config_json: String,
    shared_keys: Vec<String>,
) -> Result<i64, String> {
    let id = {
        let conn = state.db.lock().map_err(|_| "lock poisoned")?;
        let (_, sibling_json) = db::get_source_config(&conn, sibling_id).map_err(err_to_string)?;
        let merged = fill_shared_from_sibling(&config_json, &sibling_json, &shared_keys)?;
        // 既に `dpapi:` 付きの値は protect_config が二重暗号化しないので、そのまま渡してよい。
        let protected = secret::protect_config(&type_, &merged).map_err(err_to_string)?;
        db::insert_source(&conn, &type_, &name, &protected).map_err(err_to_string)?
    };
    let _ = reload_one_for_state(&state, id);
    optimize_fts(&state);
    Ok(id)
}

#[tauri::command]
pub async fn delete_source(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::delete_source(&conn, id).map_err(err_to_string)
}

/// 閲覧・編集ダイアログ用の詳細取得。マスターパスワードが未解錠なら秘密フィールドは
/// プレースホルダーで返す（要件 §9-1 / §9-4）。
#[tauri::command]
pub fn get_source_detail(state: State<'_, AppState>, id: i64) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let (type_, name, refresh_interval, config_json) =
        db::get_source_full(&conn, id).map_err(err_to_string)?;
    let unlocked = state.is_unlocked();
    let visible = if unlocked {
        secret::unprotect_config(&config_json).map_err(err_to_string)?
    } else {
        secret::mask_config(&config_json).map_err(err_to_string)?
    };
    // 保存済みトークンへの参照（cred:<id>）フィールドは、解錠中でも実値を出さず常にマスクする
    // （実体は credentials 側にあり、ソース編集画面での「解錠して編集」は意味を持たないため。
    // 誤って別の値を書き込むとリンクが意図せず外れてしまう。値を変えたい場合はトークン管理画面で）。
    let visible = crate::credentials::remask_cred_refs(&conn, &config_json, &visible)
        .map_err(err_to_string)?;
    let linked_credentials =
        crate::credentials::cred_ref_labels(&conn, &config_json).map_err(err_to_string)?;
    let config: serde_json::Value = serde_json::from_str(&visible).map_err(err_to_string)?;
    Ok(serde_json::json!({
        "type_": type_, "name": name, "refresh_interval": refresh_interval,
        "config": config, "unlocked": unlocked, "has_secret": secret::has_secret_fields(&type_),
        "linked_credentials": linked_credentials,
    }))
}

/// ソースの設定を編集保存する（要件 §9-1）。プレースホルダーのままの秘密フィールドは
/// 旧暗号化値を維持する。保存後は再取得して最新状態を反映する。
#[tauri::command]
pub async fn update_source(
    state: State<'_, AppState>,
    id: i64,
    name: String,
    config_json: String,
) -> Result<(), String> {
    {
        let conn = state.db.lock().map_err(|_| "lock poisoned")?;
        let (type_, old_config_json) = db::get_source_config(&conn, id).map_err(err_to_string)?;
        let merged = secret::merge_preserving_placeholder(&type_, &old_config_json, &config_json)
            .map_err(err_to_string)?;
        let protected = secret::protect_config(&type_, &merged).map_err(err_to_string)?;
        db::update_source(&conn, id, &name, &protected).map_err(err_to_string)?;
    }
    let _ = reload_one_for_state(&state, id);
    optimize_fts(&state);
    Ok(())
}

/// ソースを複製する（要件 §9-1）。「一部だけ変えた別ソース」を作る下準備として使う。
#[tauri::command]
pub fn duplicate_source(
    state: State<'_, AppState>,
    id: i64,
    new_name: String,
) -> Result<i64, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::duplicate_source(&conn, id, &new_name).map_err(err_to_string)
}

/// サイドバーのドラッグ並び替えを保存する（要件 §9-1）。
#[tauri::command]
pub fn reorder_sources(state: State<'_, AppState>, ordered_ids: Vec<i64>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::reorder_sources(&conn, &ordered_ids).map_err(err_to_string)
}

/// 追加ダイアログ用: まだ保存していない接続情報で試し取得し、フィールド名一覧を返す（要件 §9-3）。
/// 秘密フィールドに保存済みトークンへの参照（cred:<id>）が入っていることがあるため解決してから使う。
#[tauri::command]
pub fn preview_source_fields(
    state: State<'_, AppState>,
    type_: String,
    config_json: String,
) -> Result<Vec<String>, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let resolved =
        crate::credentials::resolve_credentials(&conn, &config_json).map_err(err_to_string)?;
    crate::connectors::raw_field_names(&type_, &resolved).map_err(err_to_string)
}

#[derive(serde::Serialize)]
pub struct FieldPreview {
    pub names: Vec<String>,
    /// true の場合、ライブ接続に失敗したためキャッシュ済みレコードから復元した名前一覧
    /// （表示名エイリアス適用後の名前になる点に注意）。
    pub from_cache: bool,
}

/// 編集ダイアログ用: 既存ソースの保存済み設定（DPAPI 暗号化のまま）で試し取得し、
/// フィールド名一覧を返す。マスターパスワードの解錠状態に関わらず動作する
/// （DPAPI 復号はアプリ内部利用であり、UI 表示ゲートとは独立のため）。
/// ライブ接続に失敗した場合（ネットワーク断・ファイル移動・認証切れ等）は、既にキャッシュ
/// 済みのレコードから列名を復元して返す（オフラインでも表示・検索設定を編集できるように）。
#[tauri::command]
pub fn preview_source_fields_by_id(
    state: State<'_, AppState>,
    id: i64,
) -> Result<FieldPreview, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let (type_, config_json) = db::get_source_config(&conn, id).map_err(err_to_string)?;
    let config_json =
        crate::credentials::resolve_credentials(&conn, &config_json).map_err(err_to_string)?;
    match crate::connectors::raw_field_names(&type_, &config_json) {
        Ok(names) => Ok(FieldPreview {
            names,
            from_cache: false,
        }),
        Err(live_err) => {
            let cached = db::cached_field_names(&conn, id).map_err(err_to_string)?;
            if cached.is_empty() {
                Err(err_to_string(live_err))
            } else {
                Ok(FieldPreview {
                    names: cached,
                    from_cache: true,
                })
            }
        }
    }
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

// ---- ブラウズして追加（要件 §9-2）----

#[tauri::command]
pub fn browse_notion_databases(token: String) -> Result<Vec<crate::browse::NotionDbEntry>, String> {
    crate::browse::notion_list_databases(&token).map_err(err_to_string)
}

/// 保存済みソースの設定（DPAPI 暗号化）を復号し、ダイアログ上の編集中の値で上書きした
/// 「実効設定」を作る。マスクされたまま（`••••••••`）や空の値は未編集とみなして
/// 保存済みの値を使う。
///
/// 接続設定ダイアログはマスターパスワードが未解錠だと秘密フィールドがマスクされて表示される。
/// その値をそのまま外部 API へ渡すと必ず認証エラーになるため、一覧取得はフロントから受け取った
/// 値ではなくこの実効設定を使う（`preview_source_fields_by_id` と同じ考え方）。
/// 保存済みトークンへの参照（cred:<id>）を実値へ展開してから復号する。
fn resolve_and_decrypt(conn: &Connection, config_json: &str) -> Result<String, String> {
    let resolved =
        crate::credentials::resolve_credentials(conn, config_json).map_err(err_to_string)?;
    secret::unprotect_config(&resolved).map_err(err_to_string)
}

fn effective_config(
    conn: &Connection,
    id: i64,
    overrides_json: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let (_, config_json) = db::get_source_config(conn, id).map_err(err_to_string)?;
    let decrypted = resolve_and_decrypt(conn, &config_json)?;
    let merged = merge_overrides(&decrypted, overrides_json)?;
    // override 自体が「このセッションで選び直した」新しい cred:<id> 参照であることがある
    // （ダイアログ上でトークンを変更しただけでまだ保存していない状態）。その場合 merge_overrides は
    // 単なる文字列として上書きするだけなので、マージ後にもう一度解決・復号する。
    let merged_json = serde_json::to_string(&merged).map_err(err_to_string)?;
    let merged_decrypted = resolve_and_decrypt(conn, &merged_json)?;
    serde_json::from_str(&merged_decrypted).map_err(err_to_string)
}

/// 復号済み設定に、ダイアログ上の編集値を重ねる。空・プレースホルダーは未編集として無視する。
fn merge_overrides(
    config_json: &str,
    overrides_json: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let mut cfg: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(config_json).map_err(err_to_string)?;
    let overrides: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(overrides_json).map_err(err_to_string)?;
    for (k, v) in overrides {
        let s = v.as_str().unwrap_or_default();
        // 空・プレースホルダーは「未編集」扱い（保存済みの値を使う）。
        // cred:<id> は「このセッションで選び直した新しいリンク」を意味する実際の編集値のため、
        // 通常の値と同様に適用する（呼び出し元の effective_config 側で改めて解決・復号する）。
        if s.is_empty() || s == secret::SECRET_PLACEHOLDER {
            continue; // 未編集 → 保存済みの値を使う
        }
        cfg.insert(k, v);
    }
    Ok(cfg)
}

fn cfg_str(cfg: &serde_json::Map<String, serde_json::Value>, key: &str) -> String {
    cfg.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

/// 接続設定ダイアログ用の Notion データベース一覧。保存済みトークンを使うため、
/// マスターパスワードが未解錠でも一覧を取得できる。
#[tauri::command]
pub fn browse_notion_databases_for_source(
    state: State<'_, AppState>,
    id: i64,
    overrides_json: String,
) -> Result<Vec<crate::browse::NotionDbEntry>, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let cfg = effective_config(&conn, id, &overrides_json)?;
    drop(conn);
    let token = cfg_str(&cfg, "token");
    if token.is_empty() {
        return Err("インテグレーショントークンが設定されていません".to_string());
    }
    crate::browse::notion_list_databases(&token).map_err(err_to_string)
}

#[tauri::command]
pub fn browse_airtable_bases(
    token: String,
) -> Result<Vec<crate::browse::AirtableBaseEntry>, String> {
    crate::browse::airtable_list_bases(&token).map_err(err_to_string)
}

/// 接続設定ダイアログ用の Airtable Base 一覧。保存済みトークンを使うため、
/// マスターパスワードが未解錠でも一覧を取得できる。
#[tauri::command]
pub fn browse_airtable_bases_for_source(
    state: State<'_, AppState>,
    id: i64,
    overrides_json: String,
) -> Result<Vec<crate::browse::AirtableBaseEntry>, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let cfg = effective_config(&conn, id, &overrides_json)?;
    drop(conn);
    let token = cfg_str(&cfg, "token");
    if token.is_empty() {
        return Err("Personal Access Token が設定されていません".to_string());
    }
    crate::browse::airtable_list_bases(&token).map_err(err_to_string)
}

#[tauri::command]
pub fn browse_airtable_tables(
    token: String,
    base_id: String,
) -> Result<Vec<crate::browse::AirtableTableEntry>, String> {
    crate::browse::airtable_list_tables(&token, &base_id).map_err(err_to_string)
}

/// 接続設定ダイアログ用の Airtable テーブル一覧。保存済みトークンを使うため、
/// マスターパスワードが未解錠でも一覧を取得できる。
#[tauri::command]
pub fn browse_airtable_tables_for_source(
    state: State<'_, AppState>,
    id: i64,
    overrides_json: String,
    base_id: Option<String>,
) -> Result<Vec<crate::browse::AirtableTableEntry>, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let cfg = effective_config(&conn, id, &overrides_json)?;
    drop(conn);
    let token = cfg_str(&cfg, "token");
    if token.is_empty() {
        return Err("Personal Access Token が設定されていません".to_string());
    }
    let target_base = match base_id {
        Some(b) if !b.is_empty() => b,
        _ => cfg_str(&cfg, "base_id"),
    };
    if target_base.is_empty() {
        return Err("Base ID が指定されていません".to_string());
    }
    crate::browse::airtable_list_tables(&token, &target_base).map_err(err_to_string)
}

/// 接続設定ダイアログ用の SharePoint リスト/ライブラリ一覧。保存済みクライアントシークレットを
/// 使うため、マスターパスワードが未解錠でも一覧を取得できる。
#[tauri::command]
pub fn browse_graph_site_resources_for_source(
    state: State<'_, AppState>,
    id: i64,
    overrides_json: String,
) -> Result<Vec<crate::browse::SiteResourceEntry>, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let cfg = effective_config(&conn, id, &overrides_json)?;
    drop(conn);
    let auth = crate::browse::GraphAuthArgs {
        tenant_id: cfg_str(&cfg, "tenant_id"),
        client_id: cfg_str(&cfg, "client_id"),
        client_secret: cfg_str(&cfg, "client_secret"),
    };
    let site_url = cfg_str(&cfg, "site_url");
    if site_url.is_empty() {
        return Err("サイト URL が設定されていません".to_string());
    }
    crate::browse::graph_list_site_resources(&auth, &site_url).map_err(err_to_string)
}

#[tauri::command]
pub fn browse_graph_sites(
    auth: crate::browse::GraphAuthArgs,
) -> Result<Vec<crate::browse::SiteEntry>, String> {
    crate::browse::graph_list_sites(&auth).map_err(err_to_string)
}

#[tauri::command]
pub fn browse_graph_teams(
    auth: crate::browse::GraphAuthArgs,
) -> Result<Vec<crate::browse::TeamEntry>, String> {
    crate::browse::graph_list_teams(&auth).map_err(err_to_string)
}

#[tauri::command]
pub fn browse_graph_team_site_url(
    auth: crate::browse::GraphAuthArgs,
    team_group_id: String,
) -> Result<String, String> {
    crate::browse::graph_team_site_url(&auth, &team_group_id).map_err(err_to_string)
}

#[tauri::command]
pub fn browse_graph_site_resources(
    auth: crate::browse::GraphAuthArgs,
    site_url: String,
) -> Result<Vec<crate::browse::SiteResourceEntry>, String> {
    crate::browse::graph_list_site_resources(&auth, &site_url).map_err(err_to_string)
}

#[tauri::command]
pub fn browse_excel_sheets(path: String) -> Result<Vec<String>, String> {
    crate::browse::list_excel_sheets(&crate::connectors::strip_path_quotes(&path))
        .map_err(err_to_string)
}

#[tauri::command]
pub fn browse_excel_tables(path: String) -> Result<Vec<String>, String> {
    crate::browse::list_excel_tables(&crate::connectors::strip_path_quotes(&path))
        .map_err(err_to_string)
}

#[tauri::command]
pub fn browse_excel_grid(
    path: String,
    sheet: String,
    rows: usize,
    cols: usize,
    mode: Option<String>,
) -> Result<crate::browse::ExcelGridPreview, String> {
    let mode_str = mode.as_deref().unwrap_or("top");
    crate::browse::preview_excel_grid(
        &crate::connectors::strip_path_quotes(&path),
        &sheet,
        rows,
        cols,
        mode_str,
    )
    .map_err(err_to_string)
}

#[tauri::command]
pub fn browse_sqlite_tables(path: String) -> Result<Vec<String>, String> {
    crate::browse::list_sqlite_tables(&crate::connectors::strip_path_quotes(&path))
        .map_err(err_to_string)
}

#[tauri::command]
pub fn browse_accdb_tables(path: String) -> Result<Vec<String>, String> {
    crate::browse::list_accdb_tables(&crate::connectors::strip_path_quotes(&path))
        .map_err(err_to_string)
}

/// ブラウザ系データソース（閲覧履歴/ブックマーク/ダウンロード履歴）のパス指定を「候補から選ぶだけ」
/// にするための一覧取得（要件: パス指定が難しいので自動で候補を探して選ばせてほしい）。
#[tauri::command]
pub fn browse_browser_profiles(
    browser: String,
    target: String,
) -> Result<Vec<crate::browse::BrowserProfileEntry>, String> {
    crate::browse::browse_browser_profiles(&browser, &target).map_err(err_to_string)
}

#[tauri::command]
pub fn set_refresh_interval(
    state: State<'_, AppState>,
    id: i64,
    seconds: i64,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::set_refresh_interval(&conn, id, seconds).map_err(err_to_string)
}

#[tauri::command]
pub fn reload_sources(state: State<'_, AppState>, source_ids: Vec<i64>) -> Result<(), String> {
    let ids = if source_ids.is_empty() {
        let conn = state.db.lock().map_err(|_| "lock poisoned")?;
        db::list_sources(&conn)
            .map_err(err_to_string)?
            .into_iter()
            .map(|source| source.id)
            .collect()
    } else {
        source_ids
    };
    for id in ids {
        let _ = reload_one_for_state(&state, id); // 個別失敗は status に記録し、続行
    }
    optimize_fts(&state);
    Ok(())
}

/// 出力ダイアログの Shift_JIS 変換用（要件: 出力の文字コード選択）。
/// ブラウザ標準の TextEncoder は Shift_JIS 等のレガシー文字コードへのエンコードに対応しないため、
/// 既存依存の encoding_rs を使ってバイト列を返す（フロントは writeFile でそのまま書き込む）。
/// 変換できない文字（機種依存文字等）は `?` に置換される（encoding_rs のエンコード仕様）。
#[tauri::command]
pub fn encode_shift_jis(text: String) -> Vec<u8> {
    let (bytes, _, _) = encoding_rs::SHIFT_JIS.encode(&text);
    bytes.into_owned()
}

#[cfg(test)]
mod parent_dialog_tests {
    use super::{fill_shared_from_sibling, merge_overrides};
    use crate::secret::SECRET_PLACEHOLDER;

    #[test]
    fn merge_overrides_ignores_masked_and_empty_values() {
        let saved = r#"{"token":"ntn_real","database_id":"db1"}"#;
        let edited = format!(r#"{{"token":"{SECRET_PLACEHOLDER}","database_id":""}}"#);
        let merged = merge_overrides(saved, &edited).unwrap();
        // マスク・空欄はユーザーが触っていない扱い → 保存済みの値が残る
        assert_eq!(merged["token"], "ntn_real");
        assert_eq!(merged["database_id"], "db1");
    }

    #[test]
    fn merge_overrides_applies_fresh_cred_reference_as_a_real_edit() {
        // cred:<id> は「ダイアログ上でこのセッション中に選び直した新しいリンク」を意味する
        // 実際の編集値であり、プレースホルダーとは違って無視してはいけない
        // （無視すると「トークンを変更した直後は反映されない」不具合になる。実際に発生した：
        // 接続設定でトークンを変更→保存前に「テーブル一覧の更新」を押すと旧トークンで検証されていた）。
        // 実際の解決（cred:<id> → 実値）は merge_overrides の外、effective_config 側の役割。
        let saved_and_resolved = r#"{"token":"ntn_old_resolved_value","database_id":"db1"}"#;
        let overrides = r#"{"token":"cred:7","database_id":"db2"}"#;
        let merged = merge_overrides(saved_and_resolved, overrides).unwrap();
        assert_eq!(merged["token"], "cred:7"); // そのまま適用される（呼び出し元が改めて解決する）
        assert_eq!(merged["database_id"], "db2");
    }

    #[test]
    fn merge_overrides_applies_edited_values() {
        let saved = r#"{"token":"ntn_old","site_url":"https://old"}"#;
        let edited = r#"{"token":"ntn_new","site_url":"https://new","extra":"x"}"#;
        let merged = merge_overrides(saved, edited).unwrap();
        assert_eq!(merged["token"], "ntn_new");
        assert_eq!(merged["site_url"], "https://new");
        assert_eq!(merged["extra"], "x");
    }

    #[test]
    fn effective_config_resolves_a_freshly_overridden_credential_reference() {
        // 再現テスト: 接続設定でトークンを選び直した直後、保存する前に「テーブル一覧の更新」を
        // 押しても新しいトークンが使われず、旧トークンで検証されているように見える不具合の回帰確認。
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::init_schema(&conn).unwrap();
        let old_id =
            crate::credentials::add_credential(&conn, "notion", "旧", r#"{"token":"ntn_old"}"#)
                .unwrap();
        let new_id =
            crate::credentials::add_credential(&conn, "notion", "新", r#"{"token":"ntn_new"}"#)
                .unwrap();
        let source_id = crate::db::insert_source(
            &conn,
            "notion",
            "テスト",
            &format!(r#"{{"token":"cred:{old_id}","database_id":"db1"}}"#),
        )
        .unwrap();

        // 保存前の override（フロントが「変更…」で選び直した直後の状態を模す）。
        let overrides = format!(r#"{{"token":"cred:{new_id}"}}"#);
        let cfg = super::effective_config(&conn, source_id, &overrides).unwrap();
        assert_eq!(cfg["token"], "ntn_new"); // 保存前でも新しく選んだトークンの実値が使われる
        assert_eq!(cfg["database_id"], "db1"); // override に無いフィールドは保存済みの値を維持

        // override が無ければ、引き続き保存済み(旧)のトークンが使われる。
        let cfg2 = super::effective_config(&conn, source_id, "{}").unwrap();
        assert_eq!(cfg2["token"], "ntn_old");
    }

    #[test]
    fn fill_shared_copies_encrypted_secret_without_decrypting() {
        let sibling = r#"{"token":"dpapi:deadbeef","database_id":"db1"}"#;
        let new_cfg = format!(r#"{{"database_id":"db2","token":"{SECRET_PLACEHOLDER}"}}"#);
        let out = fill_shared_from_sibling(&new_cfg, sibling, &["token".to_string()]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        // 暗号化済みのまま引き継ぐ（UI 側は平文を一切扱わない）
        assert_eq!(v["token"], "dpapi:deadbeef");
        // 一覧で選んだ側の値は上書きされない
        assert_eq!(v["database_id"], "db2");
    }

    /// v0.0.3 で直したバグの再現・回帰テスト。
    /// 「保存済みトークンを DPAPI で暗号化 → マスク済みの値で一覧取得」という接続設定ダイアログと
    /// 同じ経路をたどり、実際に Notion からデータベース一覧が取れることを確認する。
    #[test]
    #[ignore = "requires EVERYDB_NOTION_TOKEN"]
    fn notion_live_list_via_saved_config_even_when_masked() {
        use crate::{browse, db, secret};
        let token = std::env::var("EVERYDB_NOTION_TOKEN").expect("EVERYDB_NOTION_TOKEN");
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        let cfg = serde_json::json!({ "token": token, "database_id": "" }).to_string();
        let protected = secret::protect_config("notion", &cfg).unwrap();
        assert!(
            protected.contains("dpapi:"),
            "トークンが暗号化されていること"
        );
        let id = db::insert_source(&conn, "notion", "案件DB", &protected).unwrap();

        // ダイアログ側はロック中でマスク値しか持っていない状況を再現する。
        let overrides = format!(r#"{{"token":"{SECRET_PLACEHOLDER}"}}"#);
        let effective = super::effective_config(&conn, id, &overrides).unwrap();
        let effective_token = effective["token"].as_str().unwrap();
        assert_ne!(
            effective_token, SECRET_PLACEHOLDER,
            "保存済みの平文トークンへ解決されること"
        );

        let dbs = browse::notion_list_databases(effective_token).unwrap();
        println!("notion databases via saved config = {}", dbs.len());
        assert!(!dbs.is_empty());
    }

    #[test]
    fn fill_shared_keeps_explicitly_entered_value() {
        let sibling = r#"{"token":"dpapi:deadbeef"}"#;
        let new_cfg = r#"{"token":"ntn_typed_by_user","database_id":"db2"}"#;
        let out = fill_shared_from_sibling(new_cfg, sibling, &["token".to_string()]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["token"], "ntn_typed_by_user");
    }
}

#[cfg(test)]
mod export_encoding_tests {
    use super::encode_shift_jis;

    #[test]
    fn encodes_ascii_and_japanese_to_shift_jis_bytes() {
        // 「あ」の Shift_JIS は 0x82 0xA0。UTF-8 のまま書き出すと文字化けするため要確認。
        let bytes = encode_shift_jis("aあ".to_string());
        assert_eq!(bytes, vec![0x61, 0x82, 0xA0]);
    }
}

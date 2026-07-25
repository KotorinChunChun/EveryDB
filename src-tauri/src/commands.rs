use std::sync::Mutex;
use std::time::{Duration, Instant};

use rusqlite::Connection;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::model::{SearchResponse, SourceInfo};
use crate::refresh::reload_one;
use crate::{db, secret};

const MASTER_PASSWORD_META_KEY: &str = "master_password_hash";
const UNLOCK_DURATION: Duration = Duration::from_secs(600); // 10 分（要件 §9-4）

pub struct AppState {
    pub db: Mutex<Connection>,
    /// マスターパスワード解錠の有効期限（None または過去時刻 = ロック中）。
    unlocked_until: Mutex<Option<Instant>>,
}

impl AppState {
    pub fn new(conn: Connection) -> Self {
        Self { db: Mutex::new(conn), unlocked_until: Mutex::new(None) }
    }

    pub fn is_unlocked(&self) -> bool {
        self.unlocked_until
            .lock()
            .map(|g| g.map(|t| Instant::now() < t).unwrap_or(false))
            .unwrap_or(false)
    }

    fn unlock(&self) {
        if let Ok(mut g) = self.unlocked_until.lock() {
            *g = Some(Instant::now() + UNLOCK_DURATION);
        }
    }
}

fn err_to_string<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

// ---- マスターパスワード（要件 §9-4）----

#[tauri::command]
pub fn mp_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let is_set = db::get_meta(&conn, MASTER_PASSWORD_META_KEY).map_err(err_to_string)?.is_some();
    Ok(serde_json::json!({ "is_set": is_set, "unlocked": state.is_unlocked() }))
}

#[tauri::command]
pub fn mp_set(state: State<'_, AppState>, password: String) -> Result<(), String> {
    if password.chars().count() < 4 {
        return Err("4 文字以上で設定してください".to_string());
    }
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    if db::get_meta(&conn, MASTER_PASSWORD_META_KEY).map_err(err_to_string)?.is_some() {
        return Err("既にマスターパスワードは設定済みです".to_string());
    }
    let hash = secret::hash_master_password(&password).map_err(err_to_string)?;
    db::set_meta(&conn, MASTER_PASSWORD_META_KEY, &hash).map_err(err_to_string)?;
    drop(conn);
    state.unlock(); // 設定直後は解錠済み扱い
    Ok(())
}

#[tauri::command]
pub fn mp_verify(state: State<'_, AppState>, password: String) -> Result<bool, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let hash = db::get_meta(&conn, MASTER_PASSWORD_META_KEY)
        .map_err(err_to_string)?
        .ok_or("マスターパスワードが未設定です")?;
    drop(conn);
    let ok = secret::verify_master_password(&password, &hash).map_err(err_to_string)?;
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

#[tauri::command]
pub fn add_source(
    state: State<'_, AppState>,
    type_: String,
    name: String,
    config_json: String,
) -> Result<i64, String> {
    let mut conn = state.db.lock().map_err(|_| "lock poisoned")?;
    // 秘密情報を持つ種別（Notion/Graph 系）は、マスターパスワード設定を前提とする（要件 §9-4）。
    if secret::has_secret_fields(&type_)
        && db::get_meta(&conn, MASTER_PASSWORD_META_KEY).map_err(err_to_string)?.is_none()
    {
        return Err("このソース種別は秘密情報を扱うため、先にマスターパスワードを設定してください".to_string());
    }
    // 秘密フィールド（Notion トークン等）は DPAPI で暗号化してから保存する（要件 §4-3-bis）。
    let config_json = secret::protect_config(&type_, &config_json).map_err(err_to_string)?;
    let id = db::insert_source(&conn, &type_, &name, &config_json).map_err(err_to_string)?;
    // 追加直後に一度取得しておく（失敗してもソース自体は残り、status=error になる）。
    let _ = reload_one(&mut conn, id);
    Ok(id)
}

/// 既存の兄弟ソースから接続情報を引き継いで、同じ接続先配下にテーブルを追加する。
/// 接続設定ダイアログのテーブル一覧から追加するときに使う。
///
/// マスターパスワードが未解錠だと UI 側の秘密フィールドはマスク（`••••••••`）されているため、
/// その値で追加すると壊れたソースができてしまう。ここでは**暗号化されたままの値をコピー**する
/// ことで、UI が平文の秘密情報を扱わずに済むようにしている。
#[tauri::command]
pub fn add_source_from_sibling(
    state: State<'_, AppState>,
    sibling_id: i64,
    type_: String,
    name: String,
    config_json: String,
    shared_keys: Vec<String>,
) -> Result<i64, String> {
    let mut conn = state.db.lock().map_err(|_| "lock poisoned")?;
    if secret::has_secret_fields(&type_)
        && db::get_meta(&conn, MASTER_PASSWORD_META_KEY).map_err(err_to_string)?.is_none()
    {
        return Err("このソース種別は秘密情報を扱うため、先にマスターパスワードを設定してください".to_string());
    }
    let (_, sibling_json) = db::get_source_config(&conn, sibling_id).map_err(err_to_string)?;
    let sibling: serde_json::Value = serde_json::from_str(&sibling_json).map_err(err_to_string)?;
    let mut cfg: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&config_json).map_err(err_to_string)?;
    for k in &shared_keys {
        let cur = cfg.get(k).and_then(|v| v.as_str()).unwrap_or_default();
        if cur.is_empty() || cur == secret::SECRET_PLACEHOLDER {
            if let Some(v) = sibling.get(k) {
                cfg.insert(k.clone(), v.clone());
            }
        }
    }
    let merged = serde_json::to_string(&cfg).map_err(err_to_string)?;
    // 既に `dpapi:` 付きの値は protect_config が二重暗号化しないので、そのまま渡してよい。
    let protected = secret::protect_config(&type_, &merged).map_err(err_to_string)?;
    let id = db::insert_source(&conn, &type_, &name, &protected).map_err(err_to_string)?;
    let _ = reload_one(&mut conn, id);
    Ok(id)
}

#[tauri::command]
pub fn delete_source(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::delete_source(&conn, id).map_err(err_to_string)
}

/// 閲覧・編集ダイアログ用の詳細取得。マスターパスワードが未解錠なら秘密フィールドは
/// プレースホルダーで返す（要件 §9-1 / §9-4）。
#[tauri::command]
pub fn get_source_detail(state: State<'_, AppState>, id: i64) -> Result<serde_json::Value, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let (type_, name, refresh_interval, config_json) = db::get_source_full(&conn, id).map_err(err_to_string)?;
    let unlocked = state.is_unlocked();
    let visible = if unlocked {
        secret::unprotect_config(&config_json).map_err(err_to_string)?
    } else {
        secret::mask_config(&config_json).map_err(err_to_string)?
    };
    let config: serde_json::Value = serde_json::from_str(&visible).map_err(err_to_string)?;
    Ok(serde_json::json!({
        "type_": type_, "name": name, "refresh_interval": refresh_interval,
        "config": config, "unlocked": unlocked, "has_secret": secret::has_secret_fields(&type_),
    }))
}

/// ソースの設定を編集保存する（要件 §9-1）。プレースホルダーのままの秘密フィールドは
/// 旧暗号化値を維持する。保存後は再取得して最新状態を反映する。
#[tauri::command]
pub fn update_source(
    state: State<'_, AppState>,
    id: i64,
    name: String,
    config_json: String,
) -> Result<(), String> {
    let mut conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let (type_, old_config_json) = db::get_source_config(&conn, id).map_err(err_to_string)?;
    let merged = secret::merge_preserving_placeholder(&type_, &old_config_json, &config_json).map_err(err_to_string)?;
    let protected = secret::protect_config(&type_, &merged).map_err(err_to_string)?;
    db::update_source(&conn, id, &name, &protected).map_err(err_to_string)?;
    let _ = reload_one(&mut conn, id);
    Ok(())
}

/// ソースを複製する（要件 §9-1）。「一部だけ変えた別ソース」を作る下準備として使う。
#[tauri::command]
pub fn duplicate_source(state: State<'_, AppState>, id: i64, new_name: String) -> Result<i64, String> {
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
#[tauri::command]
pub fn preview_source_fields(type_: String, config_json: String) -> Result<Vec<String>, String> {
    crate::connectors::raw_field_names(&type_, &config_json).map_err(err_to_string)
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
pub fn preview_source_fields_by_id(state: State<'_, AppState>, id: i64) -> Result<FieldPreview, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let (type_, config_json) = db::get_source_config(&conn, id).map_err(err_to_string)?;
    match crate::connectors::raw_field_names(&type_, &config_json) {
        Ok(names) => Ok(FieldPreview { names, from_cache: false }),
        Err(live_err) => {
            let cached = db::cached_field_names(&conn, id).map_err(err_to_string)?;
            if cached.is_empty() {
                Err(err_to_string(live_err))
            } else {
                Ok(FieldPreview { names: cached, from_cache: true })
            }
        }
    }
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
fn effective_config(
    conn: &Connection,
    id: i64,
    overrides_json: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let (_, config_json) = db::get_source_config(conn, id).map_err(err_to_string)?;
    let decrypted = secret::unprotect_config(&config_json).map_err(err_to_string)?;
    let mut cfg: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&decrypted).map_err(err_to_string)?;
    let overrides: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(overrides_json).map_err(err_to_string)?;
    for (k, v) in overrides {
        let s = v.as_str().unwrap_or_default();
        if s.is_empty() || s == secret::SECRET_PLACEHOLDER {
            continue; // 未編集 → 保存済みの値を使う
        }
        cfg.insert(k, v);
    }
    Ok(cfg)
}

fn cfg_str(cfg: &serde_json::Map<String, serde_json::Value>, key: &str) -> String {
    cfg.get(key).and_then(|v| v.as_str()).unwrap_or_default().to_string()
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
pub fn browse_graph_sites(auth: crate::browse::GraphAuthArgs) -> Result<Vec<crate::browse::SiteEntry>, String> {
    crate::browse::graph_list_sites(&auth).map_err(err_to_string)
}

#[tauri::command]
pub fn browse_graph_teams(auth: crate::browse::GraphAuthArgs) -> Result<Vec<crate::browse::TeamEntry>, String> {
    crate::browse::graph_list_teams(&auth).map_err(err_to_string)
}

#[tauri::command]
pub fn browse_graph_team_site_url(auth: crate::browse::GraphAuthArgs, team_group_id: String) -> Result<String, String> {
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
    crate::browse::list_excel_sheets(&crate::connectors::strip_path_quotes(&path)).map_err(err_to_string)
}

#[tauri::command]
pub fn browse_excel_tables(path: String) -> Result<Vec<String>, String> {
    crate::browse::list_excel_tables(&crate::connectors::strip_path_quotes(&path)).map_err(err_to_string)
}

#[tauri::command]
pub fn browse_sqlite_tables(path: String) -> Result<Vec<String>, String> {
    crate::browse::list_sqlite_tables(&crate::connectors::strip_path_quotes(&path)).map_err(err_to_string)
}

#[tauri::command]
pub fn browse_accdb_tables(path: String) -> Result<Vec<String>, String> {
    crate::browse::list_accdb_tables(&crate::connectors::strip_path_quotes(&path)).map_err(err_to_string)
}

#[tauri::command]
pub fn set_refresh_interval(state: State<'_, AppState>, id: i64, seconds: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::set_refresh_interval(&conn, id, seconds).map_err(err_to_string)
}

#[tauri::command]
pub fn reload_sources(state: State<'_, AppState>, source_ids: Vec<i64>) -> Result<(), String> {
    let mut conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let ids = if source_ids.is_empty() {
        db::list_sources(&conn)
            .map_err(err_to_string)?
            .into_iter()
            .map(|s| s.id)
            .collect()
    } else {
        source_ids
    };
    for id in ids {
        let _ = reload_one(&mut conn, id); // 個別失敗は status に記録し、続行
    }
    Ok(())
}

#[tauri::command]
pub fn search(
    state: State<'_, AppState>,
    query: String,
    source_ids: Vec<i64>,
    seq: i64,
    limit: Option<i64>,
) -> Result<SearchResponse, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let limit = limit.unwrap_or(db::DEFAULT_ROW_LIMIT);
    let (rows, truncated) = db::search(&conn, &query, &source_ids, limit).map_err(err_to_string)?;
    Ok(SearchResponse { seq, rows, truncated })
}

/// 検索履歴の保存・再利用（要件 §9-5）。
#[tauri::command]
pub fn add_search_history_entry(state: State<'_, AppState>, query: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::add_search_history(&conn, &query).map_err(err_to_string)
}

#[tauri::command]
pub fn list_search_history(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::list_search_history(&conn, 15).map_err(err_to_string)
}

// ---- 検索プリセット/お気に入り（要件 REQ-BM-01）----
#[tauri::command]
pub fn save_search_preset(
    state: State<'_, AppState>,
    name: String,
    query: String,
    source_ids: Vec<i64>,
) -> Result<i64, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::save_search_preset(&conn, &name, &query, &source_ids).map_err(err_to_string)
}

#[tauri::command]
pub fn list_search_presets(state: State<'_, AppState>) -> Result<Vec<crate::model::SearchPreset>, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::list_search_presets(&conn).map_err(err_to_string)
}

#[tauri::command]
pub fn delete_search_preset(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::delete_search_preset(&conn, id).map_err(err_to_string)
}

// ---- 自動起動（要件 §9-5）----
#[tauri::command]
pub fn autostart_status(app: AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(err_to_string)
}

#[tauri::command]
pub fn autostart_set(app: AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    if enabled {
        app.autolaunch().enable().map_err(err_to_string)
    } else {
        app.autolaunch().disable().map_err(err_to_string)
    }
}

/// 「開く」で実行を許さない拡張子。open_ref の対象文字列はデータソース側の内容
/// （CSV の 1 セル等）に由来するため、汚染されたデータで実行ファイルが起動されうる。
/// ShellExecute に渡す前にここで弾く（要件外の防御。ドキュメント・データ系は従来通り開ける）。
const DANGEROUS_OPEN_EXTS: &[&str] = &[
    "exe", "com", "scr", "pif", "bat", "cmd", "msi", "msp", "cpl", "hta", "jar", "reg", "lnk",
    "ps1", "psm1", "vbs", "vbe", "js", "jse", "wsf", "wsh", "sh", "url", "inf", "chm",
];

/// 対象パスの拡張子が実行系なら Err を返す。
fn reject_executable_target(target: &str) -> Result<(), String> {
    let ext = std::path::Path::new(target)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    if let Some(ext) = ext {
        if DANGEROUS_OPEN_EXTS.contains(&ext.as_str()) {
            return Err(format!(
                "安全のため、実行ファイル形式（.{ext}）はアプリからは開きません。必要ならエクスプローラーから開いてください"
            ));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn open_ref(app: AppHandle, target: String) -> Result<(), String> {
    let is_url = target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("mailto:");
    if is_url {
        app.opener().open_url(target, None::<&str>).map_err(err_to_string)
    } else {
        reject_executable_target(&target)?;
        app.opener().open_path(target, None::<&str>).map_err(err_to_string)
    }
}

#[cfg(test)]
mod open_ref_tests {
    use super::reject_executable_target;

    #[test]
    fn rejects_executables_regardless_of_case() {
        assert!(reject_executable_target(r"\\attacker\share\payload.EXE").is_err());
        assert!(reject_executable_target(r"C:\tmp\a.bat").is_err());
        assert!(reject_executable_target(r"C:\tmp\a.ps1").is_err());
        assert!(reject_executable_target(r"C:\tmp\shortcut.lnk").is_err());
    }

    #[test]
    fn allows_ordinary_documents_and_extensionless_paths() {
        assert!(reject_executable_target(r"C:\data\customers.csv").is_ok());
        assert!(reject_executable_target(r"C:\data\employees.xlsx").is_ok());
        assert!(reject_executable_target(r"C:\data\folder").is_ok());
    }
}

// ---- UI 設定: ズーム / テーマ（要件 REQ-UI-01 / REQ-UI-02）----
const ZOOM_META_KEY: &str = "ui_zoom";
const THEME_META_KEY: &str = "ui_theme";
const DEFAULT_ZOOM: f64 = 1.0;

/// ウィンドウのズーム倍率を設定し、次回起動用にローカル設定へ保存する。
#[tauri::command]
pub fn set_zoom(app: AppHandle, state: State<'_, AppState>, factor: f64) -> Result<(), String> {
    let factor = factor.clamp(0.5, 3.0);
    if let Some(w) = app.get_webview_window("main") {
        w.set_zoom(factor).map_err(err_to_string)?;
    }
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::set_meta(&conn, ZOOM_META_KEY, &factor.to_string()).map_err(err_to_string)
}

#[tauri::command]
pub fn get_zoom(state: State<'_, AppState>) -> Result<f64, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    Ok(db::get_meta(&conn, ZOOM_META_KEY)
        .map_err(err_to_string)?
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_ZOOM))
}

/// テーマ設定を保存する（実際の適用はフロント側で CSS の data-theme 切り替えにより行う）。
#[tauri::command]
pub fn set_theme(state: State<'_, AppState>, theme: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::set_meta(&conn, THEME_META_KEY, &theme).map_err(err_to_string)
}

#[tauri::command]
pub fn get_theme(state: State<'_, AppState>) -> Result<String, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    Ok(db::get_meta(&conn, THEME_META_KEY).map_err(err_to_string)?.unwrap_or_else(|| "dark".to_string()))
}

// ---- UI 設定: 起動ショートカットキー（要件 §9-5）----
const HOTKEY_META_KEY: &str = "ui_hotkey";
const DEFAULT_HOTKEY: &str = "Alt+Ctrl+D";

/// 起動ショートカットキーを設定・変更し、永続化する。
#[tauri::command]
pub fn set_hotkey(app: AppHandle, state: State<'_, AppState>, hotkey: String) -> Result<(), String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let trimmed = hotkey.trim();
    if trimmed.is_empty() {
        return Err("ショートカットキーが空です".to_string());
    }
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    let old = db::get_meta(&conn, HOTKEY_META_KEY)
        .map_err(err_to_string)?
        .unwrap_or_else(|| DEFAULT_HOTKEY.to_string());

    // 既存のショートカットを全て解除し、新ショートカットを登録試行する
    let _ = app.global_shortcut().unregister_all();
    if let Err(e) = app.global_shortcut().register(trimmed) {
        // 失敗時は旧キーを再登録してエラーを返す
        let _ = app.global_shortcut().register(old.as_str());
        return Err(format!("キーの登録に失敗しました: {}", e));
    }
    db::set_meta(&conn, HOTKEY_META_KEY, trimmed).map_err(err_to_string)
}

#[tauri::command]
pub fn get_hotkey(state: State<'_, AppState>) -> Result<String, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    Ok(db::get_meta(&conn, HOTKEY_META_KEY)
        .map_err(err_to_string)?
        .unwrap_or_else(|| DEFAULT_HOTKEY.to_string()))
}

// ---- UI 設定: サイドバー幅（左右分割のドラッグリサイズ）----
const SIDEBAR_WIDTH_META_KEY: &str = "ui_sidebar_width";
const DEFAULT_SIDEBAR_WIDTH: i64 = 240;

#[tauri::command]
pub fn set_sidebar_width(state: State<'_, AppState>, width: i64) -> Result<(), String> {
    let width = width.clamp(120, 800);
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::set_meta(&conn, SIDEBAR_WIDTH_META_KEY, &width.to_string()).map_err(err_to_string)
}

#[tauri::command]
pub fn get_sidebar_width(state: State<'_, AppState>) -> Result<i64, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    Ok(db::get_meta(&conn, SIDEBAR_WIDTH_META_KEY)
        .map_err(err_to_string)?
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_SIDEBAR_WIDTH))
}

// ---- 親（ファイル/サーバー）ノードの表示名オーバーライド ----
#[tauri::command]
pub fn list_parent_labels(state: State<'_, AppState>) -> Result<Vec<(String, String)>, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::list_parent_labels(&conn).map_err(err_to_string)
}

#[tauri::command]
pub fn set_parent_label(state: State<'_, AppState>, parent_key: String, label: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::set_parent_label(&conn, &parent_key, &label).map_err(err_to_string)
}

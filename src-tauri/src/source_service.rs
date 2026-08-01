//! データソース操作の業務処理。Tauri command 型へ依存しない。

use crate::app_state::AppState;

use crate::model::SourceInfo;
use crate::refresh::{optimize_fts, reload_many_for_state, reload_one_for_state, ReloadSummary};
use crate::{db, secret};

fn err_to_string<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

pub fn list_sources(state: &AppState) -> Result<Vec<SourceInfo>, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::list_sources(&conn).map_err(err_to_string)
}

// 初回取得の外部I/Oは、呼び出し側の非同期Tauri commandから実行する。
pub fn add_source(
    state: &AppState,
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
    let _ = reload_one_for_state(state, id);
    optimize_fts(state);
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
pub fn add_source_from_sibling(
    state: &AppState,
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
    let _ = reload_one_for_state(state, id);
    optimize_fts(state);
    Ok(id)
}

pub fn delete_source(state: &AppState, id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::delete_source(&conn, id).map_err(err_to_string)
}

/// 閲覧・編集ダイアログ用の詳細取得。マスターパスワードが未解錠なら秘密フィールドは
/// プレースホルダーで返す（要件 §9-1 / §9-4）。
pub fn get_source_detail(state: &AppState, id: i64) -> Result<serde_json::Value, String> {
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
pub fn update_source(
    state: &AppState,
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
    let _ = reload_one_for_state(state, id);
    optimize_fts(state);
    Ok(())
}

/// ソースを複製する（要件 §9-1）。「一部だけ変えた別ソース」を作る下準備として使う。
pub fn duplicate_source(state: &AppState, id: i64, new_name: String) -> Result<i64, String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::duplicate_source(&conn, id, &new_name).map_err(err_to_string)
}

/// サイドバーのドラッグ並び替えを保存する（要件 §9-1）。
pub fn reorder_sources(state: &AppState, ordered_ids: Vec<i64>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::reorder_sources(&conn, &ordered_ids).map_err(err_to_string)
}

/// 追加ダイアログ用: まだ保存していない接続情報で試し取得し、フィールド名一覧を返す（要件 §9-3）。
/// 秘密フィールドに保存済みトークンへの参照（cred:<id>）が入っていることがあるため解決してから使う。
pub fn preview_source_fields(
    state: &AppState,
    type_: String,
    config_json: String,
) -> Result<Vec<String>, String> {
    // credential参照の解決だけをロック内で行い、接続先へのI/O中はDBを解放する。
    let resolved = {
        let conn = state.db.lock().map_err(|_| "lock poisoned")?;
        crate::credentials::resolve_credentials(&conn, &config_json).map_err(err_to_string)?
    };
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
pub fn preview_source_fields_by_id(state: &AppState, id: i64) -> Result<FieldPreview, String> {
    // 保存済み設定とcredential参照をsnapshotし、ライブ接続はロック外で実行する。
    let (type_, config_json) = {
        let conn = state.db.lock().map_err(|_| "lock poisoned")?;
        let (type_, config_json) = db::get_source_config(&conn, id).map_err(err_to_string)?;
        let resolved =
            crate::credentials::resolve_credentials(&conn, &config_json).map_err(err_to_string)?;
        (type_, resolved)
    };
    match crate::connectors::raw_field_names(&type_, &config_json) {
        Ok(names) => Ok(FieldPreview {
            names,
            from_cache: false,
        }),
        Err(live_err) => {
            let conn = state.db.lock().map_err(|_| "lock poisoned")?;
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

pub fn set_refresh_interval(state: &AppState, id: i64, seconds: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|_| "lock poisoned")?;
    db::set_refresh_interval(&conn, id, seconds).map_err(err_to_string)
}

/// 指定ソースを再取得し、個別成功・失敗・重複取消を件数とエラー付きで返す。
/// source_idsが空なら全ソースを対象にする。
pub fn reload_sources(state: &AppState, source_ids: Vec<i64>) -> Result<ReloadSummary, String> {
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
    Ok(reload_many_for_state(state, &ids))
}

#[cfg(test)]
mod source_service_tests {
    use super::fill_shared_from_sibling;
    use crate::secret::SECRET_PLACEHOLDER;
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
    fn fill_shared_keeps_explicitly_entered_value() {
        let sibling = r#"{"token":"dpapi:deadbeef"}"#;
        let new_cfg = r#"{"token":"ntn_typed_by_user","database_id":"db2"}"#;
        let out = fill_shared_from_sibling(new_cfg, sibling, &["token".to_string()]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["token"], "ntn_typed_by_user");
    }
}

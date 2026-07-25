//! 開発・検証用の一括登録ツール。
//! samples/ 以下のローカルサンプル（CSV/Excel/フォルダ/IME辞書/accdb）は認証情報不要で
//! 無条件に登録する。加えて Notion / SharePoint / EntraID / Teams のように毎回手入力が
//! 面倒な設定は、環境変数が設定されていれば登録する。
//! いずれも実際にアプリが使うキャッシュ DB（%APPDATA%\com.everydb.app\cache.db）へ
//! add_source と全く同じ経路（DPAPI 暗号化 → 初回 fetch）で登録する。
//!
//! 秘密情報はこのファイルに書かず、実行時の環境変数からのみ受け取る。
//! 実行例（PowerShell、ローカルサンプルのみなら環境変数は不要）:
//!   cargo run --example seed_test_sources
//! クラウド系も含める場合:
//!   $env:EVERYDB_NOTION_TOKEN='...'; $env:EVERYDB_NOTION_DB='...';
//!   $env:EVERYDB_GRAPH_TENANT='...'; $env:EVERYDB_GRAPH_CLIENT='...'; $env:EVERYDB_GRAPH_SECRET='...';
//!   $env:EVERYDB_SP_SITE='https://.../sites/xxx';
//!   cargo run --example seed_test_sources

use everydb_lib::{db, refresh, secret};
use rusqlite::Connection;

fn cache_db_path() -> std::path::PathBuf {
    let appdata = std::env::var("APPDATA").expect("APPDATA が取得できません");
    std::path::Path::new(&appdata).join("com.everydb.app").join("cache.db")
}

/// 同名・同種のソースが既に存在すれば削除してから追加する（再実行しても重複しない）。
fn upsert_source(conn: &mut Connection, type_: &str, name: &str, config_json: serde_json::Value) {
    let existing = db::list_sources(conn).unwrap();
    if let Some(s) = existing.iter().find(|s| s.type_ == type_ && s.name == name) {
        db::delete_source(conn, s.id).unwrap();
        println!("既存の「{name}」を削除して再登録します");
    }
    let protected = secret::protect_config(type_, &config_json.to_string()).unwrap();
    let id = db::insert_source(conn, type_, name, &protected).unwrap();
    print!("登録: {name} ({type_}) ... ");
    match refresh::reload_one(conn, id) {
        Ok(()) => {
            let count = db::list_sources(conn).unwrap().into_iter().find(|s| s.id == id).map(|s| s.record_count).unwrap_or(0);
            println!("OK ({count} 件)");
        }
        Err(e) => println!("失敗: {e}"),
    }
}

fn samples_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("samples")
}

fn main() {
    let path = cache_db_path();
    println!("対象 DB: {}", path.display());
    // 初回起動前(ディレクトリ未作成)でも動くよう、アプリと同じ場所を先に作っておく。
    std::fs::create_dir_all(path.parent().unwrap()).expect("キャッシュディレクトリの作成に失敗");
    let mut conn = Connection::open(&path).expect("cache.db を開けません");
    db::init_schema(&conn).unwrap();

    // --- ローカルサンプル(認証情報不要・存在すれば無条件で登録) ---
    let dir = samples_dir();
    let p = |name: &str| dir.join(name).to_string_lossy().into_owned();

    if dir.join("customers.csv").exists() {
        upsert_source(&mut conn, "csv", "サンプル: 顧客CSV", serde_json::json!({
            "path": p("customers.csv"), "has_header": true
        }));
    }
    if dir.join("employees.xlsx").exists() {
        upsert_source(&mut conn, "excel_table", "サンプル: 社員(Excel)", serde_json::json!({
            "path": p("employees.xlsx"), "sheet": "社員"
        }));
        upsert_source(&mut conn, "excel_sheets", "サンプル: シート名一覧", serde_json::json!({
            "path": p("employees.xlsx")
        }));
    }
    if dir.join("tree").exists() {
        upsert_source(&mut conn, "folder", "サンプル: フォルダツリー", serde_json::json!({
            "path": p("tree"), "recursive": true
        }));
    }
    if dir.join("msime_dict.txt").exists() {
        upsert_source(&mut conn, "ime_msime", "サンプル: MS-IME辞書", serde_json::json!({
            "path": p("msime_dict.txt")
        }));
    }
    if dir.join("google_dict.txt").exists() {
        upsert_source(&mut conn, "ime_google", "サンプル: Google日本語入力辞書", serde_json::json!({
            "path": p("google_dict.txt")
        }));
    }
    if dir.join("access_db.accdb").exists() {
        upsert_source(&mut conn, "accdb", "サンプル: 取引先(Access)", serde_json::json!({
            "path": p("access_db.accdb"), "table": "取引先"
        }));
    }

    // --- sample2/ (ユーザーが独自に配置したサンプルデータ) ---
    let dir2 = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("sample2");
    let p2 = |name: &str| dir2.join(name).to_string_lossy().into_owned();

    for (file, sheet) in [
        ("0804koukinsisyutsu.xlsx", "公金支出情報一覧表"),
        ("0805koukinsisyutsu.xlsx", "公金支出情報一覧表"),
    ] {
        if dir2.join(file).exists() {
            let name = format!("sample2: {}", file.trim_end_matches(".xlsx"));
            upsert_source(&mut conn, "excel_table", &name, serde_json::json!({
                "path": p2(file), "sheet": sheet
            }));
        }
    }

    let chinook = "Chinook_Sqlite.sqlite";
    if dir2.join(chinook).exists() {
        const CHINOOK_TABLES: &[&str] = &[
            "Album", "Artist", "Customer", "Employee", "Genre", "Invoice",
            "InvoiceLine", "MediaType", "Playlist", "PlaylistTrack", "Track",
        ];
        for table in CHINOOK_TABLES {
            let name = format!("sample2: Chinook.{table}");
            upsert_source(&mut conn, "sqlite", &name, serde_json::json!({
                "path": p2(chinook), "table": table
            }));
        }
    }

    // --- Notion ---
    if let (Ok(token), Ok(dbid)) = (std::env::var("EVERYDB_NOTION_TOKEN"), std::env::var("EVERYDB_NOTION_DB")) {
        upsert_source(&mut conn, "notion", "Notion (テスト)", serde_json::json!({
            "token": token, "database_id": dbid
        }));
    } else {
        println!("スキップ: Notion（EVERYDB_NOTION_TOKEN / EVERYDB_NOTION_DB 未設定）");
    }

    // --- Graph 系 ---
    let graph_auth = match (
        std::env::var("EVERYDB_GRAPH_TENANT"),
        std::env::var("EVERYDB_GRAPH_CLIENT"),
        std::env::var("EVERYDB_GRAPH_SECRET"),
    ) {
        (Ok(t), Ok(c), Ok(s)) => Some((t, c, s)),
        _ => {
            println!("スキップ: Graph 系（EVERYDB_GRAPH_TENANT / _CLIENT / _SECRET 未設定）");
            None
        }
    };

    if let Some((tenant, client, secret)) = graph_auth {
        let auth = serde_json::json!({ "tenant_id": tenant, "client_id": client, "client_secret": secret });

        upsert_source(&mut conn, "graph_entra_users", "EntraID ユーザー", auth.clone());
        upsert_source(&mut conn, "graph_entra_groups", "EntraID グループ", auth.clone());
        upsert_source(&mut conn, "graph_teams", "Teams チーム一覧", auth.clone());

        if let Ok(site) = std::env::var("EVERYDB_SP_SITE") {
            let mut list_cfg = auth.clone();
            list_cfg["site_url"] = serde_json::Value::String(site.clone());
            list_cfg["list_name"] = serde_json::Value::String(
                std::env::var("EVERYDB_SP_LIST").unwrap_or_else(|_| "SampleList".to_string()),
            );
            upsert_source(&mut conn, "graph_sharepoint_list", "SharePoint リスト (テスト)", list_cfg);

            let mut lib_cfg = auth.clone();
            lib_cfg["site_url"] = serde_json::Value::String(site);
            upsert_source(&mut conn, "graph_sharepoint_library", "SharePoint ドキュメントライブラリ (テスト)", lib_cfg);
        } else {
            println!("スキップ: SharePoint（EVERYDB_SP_SITE 未設定）");
        }
    }

    println!("完了。");
}

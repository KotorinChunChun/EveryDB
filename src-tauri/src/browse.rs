//! 「ブラウズして追加」用の一覧取得ヘルパー（要件 §9-2）。
//! 実際の Connector とは別に、ピッカー UI に候補を出すための軽量な一覧取得のみを行う。

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::graph::GraphClient;

/// Graph 系ピッカーで共通の認証情報。
#[derive(Deserialize)]
pub struct GraphAuthArgs {
    pub tenant_id: String,
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
    #[serde(default)]
    pub access_token: String,
}
impl GraphAuthArgs {
    fn uses_delegated_oauth(&self) -> bool {
        !self.access_token.is_empty()
    }
}

fn graph_client(auth: &GraphAuthArgs) -> Result<GraphClient> {
    if !auth.access_token.is_empty() {
        Ok(GraphClient::new_with_token(&auth.access_token))
    } else {
        GraphClient::new(&auth.tenant_id, &auth.client_id, &auth.client_secret)
    }
}

// ---- Notion: アクセス可能なデータベース一覧 ----
#[derive(Serialize)]
pub struct NotionDbEntry {
    pub id: String,
    pub title: String,
}

pub fn notion_list_databases(token: &str) -> Result<Vec<NotionDbEntry>> {
    let mut out = Vec::new();
    let mut start_cursor: Option<String> = None;
    loop {
        let mut body = serde_json::json!({
            "filter": { "property": "object", "value": "database" },
            "page_size": 100
        });
        if let Some(c) = &start_cursor {
            body["start_cursor"] = Value::String(c.clone());
        }
        let resp = ureq::post("https://api.notion.com/v1/search")
            .set("Authorization", &format!("Bearer {token}"))
            .set("Notion-Version", "2022-06-28")
            .set("Content-Type", "application/json")
            .send_json(body);
        let value: Value = match resp {
            Ok(r) => r.into_json()?,
            Err(ureq::Error::Status(code, r)) => {
                return Err(anyhow!(
                    "Notion API エラー {code}: {}",
                    r.into_string().unwrap_or_default()
                ))
            }
            Err(e) => return Err(anyhow!("Notion への接続に失敗: {e}")),
        };
        if let Some(results) = value.get("results").and_then(|v| v.as_array()) {
            for db in results {
                let id = db
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let title = db
                    .get("title")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.get("plain_text").and_then(|v| v.as_str()))
                            .collect::<Vec<_>>()
                            .join("")
                    })
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "(無題)".to_string());
                out.push(NotionDbEntry { id, title });
            }
        }
        let has_more = value
            .get("has_more")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        start_cursor = value
            .get("next_cursor")
            .and_then(|v| v.as_str())
            .map(String::from);
        if !has_more || start_cursor.is_none() {
            break;
        }
    }
    Ok(out)
}

// ---- Airtable: Base 一覧・Table/View メタデータ一覧 ----
#[derive(Serialize)]
pub struct AirtableBaseEntry {
    pub id: String,
    pub name: String,
    pub permission_level: String,
}

#[derive(Serialize, Clone)]
pub struct AirtableViewEntry {
    pub id: String,
    pub name: String,
    pub type_: String,
}

#[derive(Serialize, Clone)]
pub struct AirtableTableEntry {
    pub id: String,
    pub name: String,
    pub primary_field_id: String,
    pub views: Vec<AirtableViewEntry>,
}

/// Personal Access Token を使用して利用可能な Base 一覧を取得する。
/// ※ 注意: トークンに `schema.bases:read` スコープが必要。
pub fn airtable_list_bases(token: &str) -> Result<Vec<AirtableBaseEntry>> {
    let mut out = Vec::new();
    let mut offset: Option<String> = None;
    loop {
        let mut url = "https://api.airtable.com/v0/meta/bases".to_string();
        if let Some(off) = &offset {
            url.push_str(&format!("?offset={off}"));
        }
        let resp = ureq::get(&url)
            .set("Authorization", &format!("Bearer {token}"))
            .call();
        let value: Value = match resp {
            Ok(r) => r.into_json()?,
            Err(ureq::Error::Status(code, r)) => {
                return Err(anyhow!(
                    "Airtable API エラー {code}: {}",
                    r.into_string().unwrap_or_default()
                ))
            }
            Err(e) => return Err(anyhow!("Airtable への接続に失敗: {e}")),
        };
        if let Some(bases) = value.get("bases").and_then(|v| v.as_array()) {
            for b in bases {
                let id = b
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let name = b
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(無題 Base)")
                    .to_string();
                let permission_level = b
                    .get("permissionLevel")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if !id.is_empty() {
                    out.push(AirtableBaseEntry {
                        id,
                        name,
                        permission_level,
                    });
                }
            }
        }
        offset = value
            .get("offset")
            .and_then(|v| v.as_str())
            .map(String::from);
        if offset.is_none() {
            break;
        }
    }
    Ok(out)
}

/// 指定 Base 内の全テーブルおよび各テーブルのビュー一覧を取得する。
/// ※ 注意: トークンに `schema.bases:read` スコープが必要。
pub fn airtable_list_tables(token: &str, base_id: &str) -> Result<Vec<AirtableTableEntry>> {
    let url = format!("https://api.airtable.com/v0/meta/bases/{base_id}/tables");
    let resp = ureq::get(&url)
        .set("Authorization", &format!("Bearer {token}"))
        .call();
    let value: Value = match resp {
        Ok(r) => r.into_json()?,
        Err(ureq::Error::Status(code, r)) => {
            return Err(anyhow!(
                "Airtable API エラー {code}: {}",
                r.into_string().unwrap_or_default()
            ))
        }
        Err(e) => return Err(anyhow!("Airtable への接続に失敗: {e}")),
    };
    let mut out = Vec::new();
    if let Some(tables) = value.get("tables").and_then(|v| v.as_array()) {
        for t in tables {
            let id = t
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let name = t
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("(無題テーブル)")
                .to_string();
            let primary_field_id = t
                .get("primaryFieldId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if id.is_empty() {
                continue;
            }
            let mut views = Vec::new();
            if let Some(views_arr) = t.get("views").and_then(|v| v.as_array()) {
                for v in views_arr {
                    let vid = v
                        .get("id")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                    let vname = v
                        .get("name")
                        .and_then(|x| x.as_str())
                        .unwrap_or("(無題ビュー)")
                        .to_string();
                    let vtype = v
                        .get("type")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                    if !vid.is_empty() {
                        views.push(AirtableViewEntry {
                            id: vid,
                            name: vname,
                            type_: vtype,
                        });
                    }
                }
            }
            out.push(AirtableTableEntry {
                id,
                name,
                primary_field_id,
                views,
            });
        }
    }
    Ok(out)
}

// ---- SharePoint: サイト一覧 ----
#[derive(Serialize)]
pub struct SiteEntry {
    pub id: String,
    pub display_name: String,
    pub web_url: String,
    pub description: String,
    pub created_date_time: String,
}

fn graph_str(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}

pub fn graph_list_sites(auth: &GraphAuthArgs) -> Result<Vec<SiteEntry>> {
    let client = graph_client(auth)?;
    let url = "https://graph.microsoft.com/v1.0/sites?search=*&$select=id,displayName,webUrl,description,createdDateTime&$top=200";
    Ok(client
        .get_all(url)?
        .iter()
        .filter_map(|s| {
            let web_url = s.get("webUrl").and_then(|v| v.as_str())?.to_string();
            Some(SiteEntry {
                id: graph_str(s, "id"),
                display_name: graph_str(s, "displayName"),
                web_url,
                description: graph_str(s, "description"),
                created_date_time: graph_str(s, "createdDateTime"),
            })
        })
        .collect())
}

// ---- Teams: チーム一覧(ピッカー用の軽量版) ----
#[derive(Serialize)]
pub struct TeamEntry {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub created_date_time: String,
}

pub fn graph_list_teams(auth: &GraphAuthArgs) -> Result<Vec<TeamEntry>> {
    let client = graph_client(auth)?;
    let url = graph_teams_url(auth);
    Ok(client
        .get_all(url)?
        .iter()
        .filter_map(|t| {
            let id = t.get("id").and_then(|v| v.as_str())?.to_string();
            Some(TeamEntry {
                id,
                display_name: graph_str(t, "displayName"),
                description: graph_str(t, "description"),
                created_date_time: graph_str(t, "createdDateTime"),
            })
        })
        .collect())
}

/// 委任 OAuth ではサインインユーザー本人が直接参加している Teams のみを取得する。
/// `/groups` はテナント全体を対象にするため、従来の app-only 認証でだけ使用する。
fn graph_teams_url(auth: &GraphAuthArgs) -> &'static str {
    if auth.uses_delegated_oauth() {
        "https://graph.microsoft.com/v1.0/me/joinedTeams"
    } else {
        "https://graph.microsoft.com/v1.0/groups?$filter=resourceProvisioningOptions/Any(x:x%20eq%20'Team')&$select=id,displayName,description,createdDateTime&$top=999"
    }
}

/// Teams チームの裏側にある SharePoint サイトの URL を解決する。
pub fn graph_team_site_url(auth: &GraphAuthArgs, team_group_id: &str) -> Result<String> {
    let client = graph_client(auth)?;
    let v = client.get(&format!(
        "https://graph.microsoft.com/v1.0/groups/{team_group_id}/sites/root?$select=webUrl"
    ))?;
    v.get("webUrl")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| anyhow!("チームに紐づく SharePoint サイトを取得できません"))
}

// ---- SharePoint サイト内のリスト/ライブラリ一覧 ----
#[derive(Serialize)]
pub struct SiteResourceEntry {
    pub display_name: String,
    /// "list" または "library"。ライブラリは document library（Graph 上は list の一種）。
    pub kind: String,
}

pub fn graph_list_site_resources(
    auth: &GraphAuthArgs,
    site_url: &str,
) -> Result<Vec<SiteResourceEntry>> {
    let client = graph_client(auth)?;
    let sid = crate::graph::site_id(&client, site_url)?;
    let lists = client.get(&format!(
        "https://graph.microsoft.com/v1.0/sites/{sid}/lists?$select=name,displayName,list"
    ))?;
    let mut out = Vec::new();
    if let Some(arr) = lists.get("value").and_then(|v| v.as_array()) {
        for l in arr {
            let template = l
                .get("list")
                .and_then(|x| x.get("template"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            // 非表示システムリストは除外(拡張機能一覧・分類法隠しリスト等)。
            if matches!(template, "webTemplateExtensionsList" | "") {
                continue;
            }
            let display_name = l
                .get("displayName")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if display_name.is_empty() {
                continue;
            }
            let kind = if template == "documentLibrary" {
                "library"
            } else {
                "list"
            };
            out.push(SiteResourceEntry {
                display_name,
                kind: kind.to_string(),
            });
        }
    }
    Ok(out)
}

// ---- ローカル DB 系: テーブル/シート一覧(要件 §9-2: 一括選択追加) ----

pub fn list_excel_sheets(path: &str) -> Result<Vec<String>> {
    use calamine::Reader;
    let wb = calamine::open_workbook_auto(path)?;
    Ok(wb.sheet_names())
}

/// ブック内の Excel テーブル(ListObject)名一覧。xlsx/xlsm 形式が必要。
pub fn list_excel_tables(path: &str) -> Result<Vec<String>> {
    let mut wb: calamine::Xlsx<_> = calamine::open_workbook(path).map_err(|e| {
        anyhow!("Excel テーブル(ListObject)の読み込みには xlsx/xlsm 形式が必要です: {e}")
    })?;
    wb.load_tables()?;
    Ok(wb.table_names().into_iter().cloned().collect())
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct ExcelGridPreview {
    pub start_row: u32,
    pub start_col: u32,
    pub rows: Vec<Vec<String>>,
}

/// Excelシートの max_rows x max_cols セルを表形式グリッドでプレビュー取得する。
/// - "top": 先頭行・先頭列からの 10x10
/// - "bottom_left" / "bottom": 最終行の先頭列からの 10x10
/// - "bottom_right": 最終行の最終列からの 10x10
pub fn preview_excel_grid(
    path: &str,
    sheet: &str,
    max_rows: usize,
    max_cols: usize,
    mode: &str,
) -> Result<ExcelGridPreview> {
    use calamine::Reader;
    let mut wb = calamine::open_workbook_auto(path)?;
    let range = wb.worksheet_range(sheet)?;
    let end_pos = range.end().unwrap_or((0, 0));

    let start_row = if mode == "bottom" || mode == "bottom_left" || mode == "bottom_right" {
        end_pos
            .0
            .saturating_sub((max_rows as u32).saturating_sub(1))
    } else {
        0
    };
    let end_row = start_row + (max_rows as u32).saturating_sub(1);

    let start_col = if mode == "bottom_right" {
        end_pos
            .1
            .saturating_sub((max_cols as u32).saturating_sub(1))
    } else {
        0
    };

    let mut out = Vec::new();
    for r in start_row..=end_row {
        let mut row_vals = Vec::new();
        for c in 0..max_cols as u32 {
            let col_idx = start_col + c;
            let val = range
                .get_value((r, col_idx))
                .map(crate::connectors::cell_to_string)
                .unwrap_or_default();
            row_vals.push(val);
        }
        out.push(row_vals);
    }
    Ok(ExcelGridPreview {
        start_row,
        start_col,
        rows: out,
    })
}

pub fn list_sqlite_tables(path: &str) -> Result<Vec<String>> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )?;
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let names = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(names)
}

pub fn list_accdb_tables(path: &str) -> Result<Vec<String>> {
    use odbc_api::{ConnectionOptions, Cursor, Environment};
    let cs = format!("Driver={{Microsoft Access Driver (*.mdb, *.accdb)}};Dbq={path};");
    let env = Environment::new().map_err(|e| anyhow!("ODBC 環境の初期化に失敗: {e}"))?;
    let conn = env
        .connect_with_connection_string(&cs, ConnectionOptions::default())
        .map_err(|e| anyhow!("Access への接続に失敗: {e}"))?;
    // TablesRow(既定の tables())は narrow(ANSI) バインドのため日本語テーブル名が文字化けする。
    // preallocate + into_tables_cursor の生カーソルを使い、AccessConnector と同じ
    // get_wide_text(UTF-16) で取得することで Unicode 安全にする。
    let stmt = conn
        .preallocate()
        .map_err(|e| anyhow!("ODBC ステートメント確保に失敗: {e}"))?;
    let mut cursor = stmt
        .into_tables_cursor("", "", "", "TABLE")
        .map_err(|e| anyhow!("テーブル一覧の取得に失敗: {e}"))?;
    // 列順: TABLE_CAT=1, TABLE_SCHEM=2, TABLE_NAME=3, TABLE_TYPE=4, REMARKS=5
    let mut out = Vec::new();
    let mut buf: Vec<u16> = Vec::new();
    while let Some(mut row) = cursor.next_row()? {
        buf.clear();
        let not_null = row.get_wide_text(3, &mut buf)?;
        if !not_null {
            continue;
        }
        let name = String::from_utf16_lossy(&buf)
            .trim_end_matches('\u{0}')
            .to_string();
        // Access の内部システムテーブル(MSys*)は除外。
        if !name.starts_with("MSys") {
            out.push(name);
        }
    }
    Ok(out)
}

// ---- ブラウザのプロファイル候補（要件: パス指定が難しいので候補から選ぶだけにしたい）----
#[derive(Serialize)]
pub struct BrowserProfileEntry {
    pub label: String,
    pub path: String,
}

/// `browser` のプロファイルを列挙し、`target`（"history" | "bookmarks"）に対応するファイルが
/// 実在するものだけを返す。history と downloads は Chromium 系では同じ `History` ファイルを
/// 共有するため、呼び出し側は downloads も "history" を指定する。
pub fn browse_browser_profiles(browser: &str, target: &str) -> Result<Vec<BrowserProfileEntry>> {
    if browser == "firefox" {
        firefox_profiles()
    } else {
        chromium_profiles(browser, target)
    }
}

fn chromium_user_data_dir(browser: &str) -> Result<std::path::PathBuf> {
    let local = std::env::var("LOCALAPPDATA")
        .map_err(|_| anyhow!("環境変数 LOCALAPPDATA が取得できません"))?;
    let sub = match browser {
        "chrome" => r"Google\Chrome\User Data",
        "edge" => r"Microsoft\Edge\User Data",
        "brave" => r"BraveSoftware\Brave-Browser\User Data",
        "vivaldi" => r"Vivaldi\User Data",
        other => return Err(anyhow!("未対応のブラウザです: {other}")),
    };
    Ok(std::path::Path::new(&local).join(sub))
}

/// `Local State`（User Data 直下の JSON）からプロファイルフォルダ名 → 表示名の対応を読む。
/// 取得できなくても致命的ではない（フォルダ名をそのまま表示名にフォールバックする）。
fn read_chromium_profile_names(
    user_data: &std::path::Path,
) -> std::collections::HashMap<String, String> {
    match std::fs::read_to_string(user_data.join("Local State")) {
        Ok(text) => parse_local_state_profile_names(&text),
        Err(_) => std::collections::HashMap::new(),
    }
}

/// `Local State` の JSON テキストから `profile.info_cache.<dir>.name` を取り出す純粋関数（テスト用に分離）。
fn parse_local_state_profile_names(text: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    if let Ok(v) = serde_json::from_str::<Value>(text) {
        if let Some(cache) = v
            .get("profile")
            .and_then(|p| p.get("info_cache"))
            .and_then(|c| c.as_object())
        {
            for (dir, info) in cache {
                if let Some(name) = info.get("name").and_then(|n| n.as_str()) {
                    map.insert(dir.clone(), name.to_string());
                }
            }
        }
    }
    map
}

fn chromium_profiles(browser: &str, target: &str) -> Result<Vec<BrowserProfileEntry>> {
    let user_data = chromium_user_data_dir(browser)?;
    if !user_data.is_dir() {
        return Err(anyhow!(
            "{} が見つかりません（未インストール、または既定と異なる場所にインストールされています）",
            user_data.display()
        ));
    }
    let target_file = if target == "bookmarks" {
        "Bookmarks"
    } else {
        "History"
    };
    let names = read_chromium_profile_names(&user_data);
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&user_data)
        .map_err(|e| anyhow!("{} の読み込みに失敗: {e}", user_data.display()))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().to_string();
        let file_path = entry.path().join(target_file);
        if file_path.is_file() {
            let display_name = names
                .get(&dir_name)
                .cloned()
                .unwrap_or_else(|| dir_name.clone());
            let label = if display_name == dir_name {
                dir_name.clone()
            } else {
                format!("{display_name} ({dir_name})")
            };
            out.push(BrowserProfileEntry {
                label,
                path: file_path.to_string_lossy().to_string(),
            });
        }
    }
    out.sort_by(|a, b| a.label.cmp(&b.label));
    if out.is_empty() {
        return Err(anyhow!(
            "{target_file} を含むプロファイルが見つかりませんでした"
        ));
    }
    Ok(out)
}

/// 1 プロファイル分の `profiles.ini` の内容（Name / Path / IsRelative）。
struct IniProfile {
    name: Option<String>,
    path: Option<String>,
    is_relative: bool,
}

/// `profiles.ini` を最小限だけ解釈する（`[Profile*]` セクションの Name/Path/IsRelative）。
/// 外部クレートを増やさずに済む単純な INI 構造のため自前でパースする（ファイル I/O を含まない純粋関数）。
fn parse_profiles_ini(text: &str) -> Vec<IniProfile> {
    let mut out = Vec::new();
    let mut in_profile = false;
    let mut name: Option<String> = None;
    let mut path: Option<String> = None;
    let mut is_relative = true;
    let flush = |name: &Option<String>,
                 path: &Option<String>,
                 is_relative: bool,
                 out: &mut Vec<IniProfile>| {
        out.push(IniProfile {
            name: name.clone(),
            path: path.clone(),
            is_relative,
        });
    };
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            if in_profile {
                flush(&name, &path, is_relative, &mut out);
            }
            in_profile = line.starts_with("[Profile");
            name = None;
            path = None;
            is_relative = true;
            continue;
        }
        if !in_profile {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            match k.trim() {
                "Name" => name = Some(v.trim().to_string()),
                "Path" => path = Some(v.trim().to_string()),
                "IsRelative" => is_relative = v.trim() == "1",
                _ => {}
            }
        }
    }
    if in_profile {
        flush(&name, &path, is_relative, &mut out);
    }
    out
}

fn firefox_profiles() -> Result<Vec<BrowserProfileEntry>> {
    let appdata =
        std::env::var("APPDATA").map_err(|_| anyhow!("環境変数 APPDATA が取得できません"))?;
    let ff_root = std::path::Path::new(&appdata).join(r"Mozilla\Firefox");
    let mut out = Vec::new();

    if let Ok(text) = std::fs::read_to_string(ff_root.join("profiles.ini")) {
        for profile in parse_profiles_ini(&text) {
            let Some(p) = &profile.path else { continue };
            let full = if profile.is_relative {
                ff_root.join(p)
            } else {
                std::path::PathBuf::from(p)
            };
            let places = full.join("places.sqlite");
            if places.is_file() {
                out.push(BrowserProfileEntry {
                    label: profile.name.clone().unwrap_or_else(|| p.clone()),
                    path: places.to_string_lossy().to_string(),
                });
            }
        }
    }

    if out.is_empty() {
        // profiles.ini が無い/読めない場合のフォールバック: Profiles 直下を直接見る。
        let profiles_dir = ff_root.join("Profiles");
        if profiles_dir.is_dir() {
            for entry in std::fs::read_dir(&profiles_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    let places = entry.path().join("places.sqlite");
                    if places.is_file() {
                        out.push(BrowserProfileEntry {
                            label: entry.file_name().to_string_lossy().to_string(),
                            path: places.to_string_lossy().to_string(),
                        });
                    }
                }
            }
        }
    }

    if out.is_empty() {
        return Err(anyhow!("Firefox のプロファイルが見つかりませんでした"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn sample(name: &str) -> String {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("samples")
            .join(name)
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn graph_team_picker_uses_user_context_for_delegated_oauth() {
        let delegated = GraphAuthArgs {
            tenant_id: "tenant".to_string(),
            client_id: "client".to_string(),
            client_secret: String::new(),
            access_token: "delegated-token".to_string(),
        };
        assert_eq!(
            graph_teams_url(&delegated),
            "https://graph.microsoft.com/v1.0/me/joinedTeams"
        );
    }

    #[test]
    fn graph_team_picker_keeps_tenant_query_for_app_only_auth() {
        let app_only = GraphAuthArgs {
            tenant_id: "tenant".to_string(),
            client_id: "client".to_string(),
            client_secret: "secret".to_string(),
            access_token: String::new(),
        };
        assert!(graph_teams_url(&app_only).contains("/groups?"));
    }

    #[test]
    fn list_excel_sheets_returns_all_sheet_names() {
        let names = list_excel_sheets(&sample("employees.xlsx")).unwrap();
        assert_eq!(names, vec!["社員", "テーブルサンプル", "拠点", "設定"]);
    }

    #[test]
    fn list_excel_tables_returns_listobject_names() {
        let names = list_excel_tables(&sample("employees.xlsx")).unwrap();
        assert!(names.contains(&"社員テーブル".to_string()));
    }

    #[test]
    fn preview_excel_grid_returns_grid() {
        let grid_top = preview_excel_grid(&sample("employees.xlsx"), "社員", 5, 5, "top").unwrap();
        assert_eq!(grid_top.start_row, 0);
        assert_eq!(grid_top.start_col, 0);
        assert_eq!(grid_top.rows.len(), 5);
        assert_eq!(grid_top.rows[0].len(), 5);

        let grid_bottom =
            preview_excel_grid(&sample("employees.xlsx"), "社員", 5, 5, "bottom").unwrap();
        assert_eq!(grid_bottom.rows.len(), 5);
    }

    #[test]
    fn list_sqlite_tables_excludes_sqlite_internal() {
        let tmp = std::env::temp_dir().join("everydb_list_sqlite_tables_test.db");
        let _ = std::fs::remove_file(&tmp);
        {
            let conn = rusqlite::Connection::open(&tmp).unwrap();
            conn.execute("CREATE TABLE foo(id INTEGER)", []).unwrap();
            conn.execute("CREATE TABLE bar(id INTEGER)", []).unwrap();
        }
        let names = list_sqlite_tables(tmp.to_str().unwrap()).unwrap();
        assert_eq!(names, vec!["bar", "foo"]);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    #[ignore = "requires Microsoft Access ODBC driver + samples/access_db.accdb"]
    fn list_accdb_tables_finds_test_table() {
        let names = list_accdb_tables(&sample("access_db.accdb")).unwrap();
        assert!(names.contains(&"取引先".to_string()));
        assert!(!names.iter().any(|n| n.starts_with("MSys")));
    }

    fn env(k: &str) -> Option<String> {
        std::env::var(k).ok().filter(|s| !s.is_empty())
    }

    #[test]
    #[ignore = "requires EVERYDB_NOTION_TOKEN"]
    fn notion_live_list_databases() {
        let token = env("EVERYDB_NOTION_TOKEN").expect("EVERYDB_NOTION_TOKEN");
        let dbs = notion_list_databases(&token).unwrap();
        println!("notion databases = {}", dbs.len());
        assert!(!dbs.is_empty());
    }

    #[test]
    #[ignore = "requires EVERYDB_GRAPH_* + EVERYDB_SP_SITE env vars"]
    fn graph_live_browse_all() {
        let auth = GraphAuthArgs {
            tenant_id: env("EVERYDB_GRAPH_TENANT").expect("EVERYDB_GRAPH_TENANT"),
            client_id: env("EVERYDB_GRAPH_CLIENT").expect("EVERYDB_GRAPH_CLIENT"),
            client_secret: env("EVERYDB_GRAPH_SECRET").expect("EVERYDB_GRAPH_SECRET"),
            access_token: String::new(),
        };
        let sites = graph_list_sites(&auth).unwrap();
        println!("sites = {}", sites.len());
        assert!(!sites.is_empty());

        let teams = graph_list_teams(&auth).unwrap();
        println!("teams = {}", teams.len());
        assert!(!teams.is_empty());

        let team_site = graph_team_site_url(&auth, &teams[0].id).unwrap();
        println!("team[0] site url = {team_site}");
        assert!(team_site.starts_with("http"));

        if let Some(site) = env("EVERYDB_SP_SITE") {
            let resources = graph_list_site_resources(&auth, &site).unwrap();
            println!("site resources = {}", resources.len());
            assert!(!resources.is_empty());
            assert!(resources.iter().any(|r| r.kind == "list"));
            assert!(resources.iter().any(|r| r.kind == "library"));
        }
    }

    #[test]
    fn parse_local_state_profile_names_reads_info_cache() {
        let json = r#"{"profile":{"info_cache":{"Default":{"name":"仕事用"},"Profile 1":{"name":"個人用"}}}}"#;
        let names = parse_local_state_profile_names(json);
        assert_eq!(names.get("Default").map(String::as_str), Some("仕事用"));
        assert_eq!(names.get("Profile 1").map(String::as_str), Some("個人用"));
    }

    #[test]
    fn parse_local_state_profile_names_empty_on_malformed_json() {
        assert!(parse_local_state_profile_names("not json").is_empty());
        assert!(parse_local_state_profile_names("{}").is_empty());
    }

    #[test]
    fn parse_profiles_ini_reads_name_path_and_relative_flag() {
        let ini = "\
[General]
StartWithLastProfile=1

[Profile1]
Name=既定プロファイル
IsRelative=1
Path=xxxxxxxx.default-release

[Profile0]
Name=別プロファイル
IsRelative=0
Path=D:\\FirefoxData\\other
";
        let profiles = parse_profiles_ini(ini);
        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0].name.as_deref(), Some("既定プロファイル"));
        assert_eq!(
            profiles[0].path.as_deref(),
            Some("xxxxxxxx.default-release")
        );
        assert!(profiles[0].is_relative);
        assert_eq!(profiles[1].name.as_deref(), Some("別プロファイル"));
        assert!(!profiles[1].is_relative);
    }

    #[test]
    fn parse_profiles_ini_ignores_non_profile_sections() {
        let ini = "[General]\nName=これは無視される\nPath=これも無視\n";
        let profiles = parse_profiles_ini(ini);
        assert!(profiles.is_empty());
    }

    #[test]
    fn chromium_user_data_dir_rejects_unknown_browser() {
        assert!(chromium_user_data_dir("safari").is_err());
    }

    // 実機の Chrome/Edge/Firefox プロファイルが必要なライブ検証。
    // 実行: cargo test -- --ignored browser_profiles_live
    #[test]
    #[ignore = "requires a real Chrome/Edge/Firefox profile on this machine"]
    fn browser_profiles_live() {
        let chrome = browse_browser_profiles("chrome", "history").unwrap();
        println!("chrome profiles (history) = {}", chrome.len());
        assert!(!chrome.is_empty());

        let edge_bm = browse_browser_profiles("edge", "bookmarks").unwrap();
        println!("edge profiles (bookmarks) = {}", edge_bm.len());
        assert!(!edge_bm.is_empty());
    }
}

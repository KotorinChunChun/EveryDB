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
    pub client_secret: String,
}

fn graph_client(auth: &GraphAuthArgs) -> Result<GraphClient> {
    GraphClient::new(&auth.tenant_id, &auth.client_id, &auth.client_secret)
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
                return Err(anyhow!("Notion API エラー {code}: {}", r.into_string().unwrap_or_default()))
            }
            Err(e) => return Err(anyhow!("Notion への接続に失敗: {e}")),
        };
        if let Some(results) = value.get("results").and_then(|v| v.as_array()) {
            for db in results {
                let id = db.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
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
        let has_more = value.get("has_more").and_then(|v| v.as_bool()).unwrap_or(false);
        start_cursor = value.get("next_cursor").and_then(|v| v.as_str()).map(String::from);
        if !has_more || start_cursor.is_none() {
            break;
        }
    }
    Ok(out)
}

// ---- SharePoint: サイト一覧 ----
#[derive(Serialize)]
pub struct SiteEntry {
    pub web_url: String,
    pub display_name: String,
}

pub fn graph_list_sites(auth: &GraphAuthArgs) -> Result<Vec<SiteEntry>> {
    let client = graph_client(auth)?;
    let url = "https://graph.microsoft.com/v1.0/sites?search=*&$select=id,displayName,webUrl&$top=200";
    Ok(client
        .get_all(url)?
        .iter()
        .filter_map(|s| {
            let web_url = s.get("webUrl").and_then(|v| v.as_str())?.to_string();
            let display_name = s.get("displayName").and_then(|v| v.as_str()).unwrap_or("").to_string();
            Some(SiteEntry { web_url, display_name })
        })
        .collect())
}

// ---- Teams: チーム一覧(ピッカー用の軽量版) ----
#[derive(Serialize)]
pub struct TeamEntry {
    pub group_id: String,
    pub display_name: String,
}

pub fn graph_list_teams(auth: &GraphAuthArgs) -> Result<Vec<TeamEntry>> {
    let client = graph_client(auth)?;
    let url = "https://graph.microsoft.com/v1.0/groups?$filter=resourceProvisioningOptions/Any(x:x%20eq%20'Team')&$select=id,displayName&$top=999";
    Ok(client
        .get_all(url)?
        .iter()
        .filter_map(|t| {
            let group_id = t.get("id").and_then(|v| v.as_str())?.to_string();
            let display_name = t.get("displayName").and_then(|v| v.as_str()).unwrap_or("").to_string();
            Some(TeamEntry { group_id, display_name })
        })
        .collect())
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

pub fn graph_list_site_resources(auth: &GraphAuthArgs, site_url: &str) -> Result<Vec<SiteResourceEntry>> {
    let client = graph_client(auth)?;
    let sid = crate::graph::site_id(&client, site_url)?;
    let lists = client.get(&format!(
        "https://graph.microsoft.com/v1.0/sites/{sid}/lists?$select=name,displayName,list"
    ))?;
    let mut out = Vec::new();
    if let Some(arr) = lists.get("value").and_then(|v| v.as_array()) {
        for l in arr {
            let template = l.get("list").and_then(|x| x.get("template")).and_then(|v| v.as_str()).unwrap_or("");
            // 非表示システムリストは除外(拡張機能一覧・分類法隠しリスト等)。
            if matches!(template, "webTemplateExtensionsList" | "" ) {
                continue;
            }
            let display_name = l.get("displayName").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if display_name.is_empty() {
                continue;
            }
            let kind = if template == "documentLibrary" { "library" } else { "list" };
            out.push(SiteResourceEntry { display_name, kind: kind.to_string() });
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
    let mut wb: calamine::Xlsx<_> = calamine::open_workbook(path)
        .map_err(|e| anyhow!("Excel テーブル(ListObject)の読み込みには xlsx/xlsm 形式が必要です: {e}"))?;
    wb.load_tables()?;
    Ok(wb.table_names().into_iter().cloned().collect())
}

pub fn list_sqlite_tables(path: &str) -> Result<Vec<String>> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )?;
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let names = stmt.query_map([], |r| r.get::<_, String>(0))?.filter_map(|r| r.ok()).collect();
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
    let stmt = conn.preallocate().map_err(|e| anyhow!("ODBC ステートメント確保に失敗: {e}"))?;
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
        let name = String::from_utf16_lossy(&buf).trim_end_matches('\u{0}').to_string();
        // Access の内部システムテーブル(MSys*)は除外。
        if !name.starts_with("MSys") {
            out.push(name);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn sample(name: &str) -> String {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("samples").join(name).to_string_lossy().into_owned()
    }

    #[test]
    fn list_excel_sheets_returns_all_sheet_names() {
        let names = list_excel_sheets(&sample("employees.xlsx")).unwrap();
        assert_eq!(names, vec!["社員", "テーブルサンプル", "拠点", "設定"]);
    }

    #[test]
    fn list_excel_tables_returns_listobject_names() {
        let names = list_excel_tables(&sample("employees.xlsx")).unwrap();
        assert_eq!(names, vec!["社員テーブル".to_string()]);
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
        };
        let sites = graph_list_sites(&auth).unwrap();
        println!("sites = {}", sites.len());
        assert!(!sites.is_empty());

        let teams = graph_list_teams(&auth).unwrap();
        println!("teams = {}", teams.len());
        assert!(!teams.is_empty());

        let team_site = graph_team_site_url(&auth, &teams[0].group_id).unwrap();
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
}

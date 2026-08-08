#[cfg(test)]
use crate::model::Record;
use anyhow::Result;
#[cfg(test)]
use serde::Deserialize;
use serde_json::Value;

mod cloud;
mod common;
mod database;
mod factory;
mod graph_data;
pub(crate) mod limits;
mod local;
mod public_data;

pub use factory::{build_connector, is_supported_source_type, Connector, SUPPORTED_SOURCE_TYPES};
pub use limits::{DB_FETCH_LIMIT, MAX_FETCH_RECORDS};

pub(crate) use graph_data::{json_scalar_to_string, urlencoding_light};
pub(crate) use local::{cell_to_string, col_to_letters};

#[cfg(test)]
use cloud::{
    airtable_value_to_string, ical_unescape, ical_unfold, notion_page_to_record,
    notion_prop_to_string, parse_ical_datetime,
};
#[cfg(test)]
use common::CommonCfg;
#[cfg(test)]
use graph_data::{graph_flat_record, mailto_open_ref};
#[cfg(test)]
use local::{
    basename_of, file_uri_to_path, fmt_epoch, parse_ref_columns, webkit_micros_to_unix_secs,
};
pub(crate) fn default_true() -> bool {
    true
}

/// パス文字列の両端のダブルクォートを除去する（エクスプローラの「パスのコピー」対策）。
pub fn strip_path_quotes(s: &str) -> String {
    let t = s.trim();
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        t[1..t.len() - 1].trim().to_string()
    } else {
        t.to_string()
    }
}

/// フィールド役割設定(display/searchable/label/open_ref/alias)を取り除いた素の config で
/// fetch し、返ってきた全レコードのフィールド名を出現順・重複無しで返す(プレビュー用。要件 §9-3)。
const ROLE_KEYS: &[&str] = &[
    "display_columns",
    "column_aliases",
    "searchable_columns",
    "label_column",
    "open_ref_column",
    "column_order",
];

pub fn raw_field_names(type_: &str, config_json: &str) -> Result<Vec<String>> {
    let mut v: Value = serde_json::from_str(config_json)?;
    if let Some(obj) = v.as_object_mut() {
        for k in ROLE_KEYS {
            obj.remove(*k);
        }
    }
    let stripped = serde_json::to_string(&v)?;
    let connector = build_connector(type_, &stripped)?;
    connector.field_names()
}

#[cfg(test)]
#[derive(Deserialize)]
struct SourceTypeManifestEntry {
    id: String,
    browse: Option<String>,
    open: String,
    guide: String,
}

#[cfg(test)]
#[test]
fn source_type_manifest_matches_factory_registry() {
    let manifest: Vec<SourceTypeManifestEntry> =
        serde_json::from_str(include_str!("../../../shared/source-types.json"))
            .expect("shared source type manifest must be valid JSON");
    let manifest_ids: Vec<&str> = manifest.iter().map(|entry| entry.id.as_str()).collect();

    assert_eq!(manifest_ids, SUPPORTED_SOURCE_TYPES);
    for entry in &manifest {
        assert!(
            matches!(
                entry.browse.as_deref(),
                None | Some(
                    "notion" | "sqlite" | "accdb" | "excel_table" | "sharepoint" | "airtable"
                )
            ),
            "unknown browse capability for {}",
            entry.id
        );
        assert!(
            matches!(
                entry.open.as_str(),
                "none" | "table" | "parent" | "table-and-parent"
            ),
            "unknown open capability for {}",
            entry.id
        );
        assert!(
            !entry.guide.trim().is_empty(),
            "guide is missing for {}",
            entry.id
        );
    }
}
#[cfg(test)]
#[test]
fn every_registered_source_type_has_a_factory_branch() {
    for type_ in SUPPORTED_SOURCE_TYPES {
        match build_connector(type_, "{}") {
            Ok(_) => {}
            Err(error) => assert!(
                !error.to_string().contains("factory定義が不足"),
                "factory branch is missing for {type_}: {error}"
            ),
        }
    }
    assert!(is_supported_source_type("airtable"));
    assert!(!is_supported_source_type("unknown"));
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

    fn fetch(type_: &str, cfg: serde_json::Value) -> Vec<Record> {
        build_connector(type_, &cfg.to_string())
            .unwrap()
            .fetch()
            .unwrap()
    }

    #[test]
    fn csv_skip_rows_top_and_bottom() {
        let base = fetch(
            "csv",
            serde_json::json!({ "path": sample("customers.csv") }),
        )
        .len();
        assert!(base >= 2);
        // 末尾1行スキップ → データが1行減る。
        let bottom = fetch(
            "csv",
            serde_json::json!({ "path": sample("customers.csv"), "skip_rows_bottom": 1 }),
        );
        assert_eq!(bottom.len(), base - 1);
        // 先頭1行スキップ → 本来のヘッダーが飛び、次行がヘッダーに昇格するのでデータは1行減る。
        let top = fetch(
            "csv",
            serde_json::json!({ "path": sample("customers.csv"), "skip_rows_top": 1 }),
        );
        assert_eq!(top.len(), base - 1);
        // 1行目データ「田中太郎」はヘッダーに昇格したのでレコードには現れない。
        assert!(!top
            .iter()
            .any(|r| r.fields.values().any(|v| v.as_str() == Some("田中太郎"))));
    }

    #[test]
    fn excel_table_reads_listobject_rows() {
        // 「テーブル」= Excel の構造化テーブル(ListObject)。同一シート内の生セル表とは別領域。
        let recs = fetch(
            "excel_table",
            serde_json::json!({ "path": sample("employees.xlsx"), "table": "社員テーブル" }),
        );
        assert_eq!(recs.len(), 3);
        assert!(recs
            .iter()
            .any(|r| r.fields.get("氏名").and_then(|v| v.as_str()) == Some("山本次郎")));
        // ListObject の列だけが読まれ、隣接する生セル表の列は含まれない。
        assert_eq!(recs[0].fields.len(), 3);
    }

    #[test]
    fn excel_table_legacy_sheet_key_still_works() {
        // table キー未設定・sheet キーのみの旧データはシート全体読みへ後方互換フォールバックする。
        let recs = fetch(
            "excel_table",
            serde_json::json!({ "path": sample("employees.xlsx"), "sheet": "社員" }),
        );
        assert_eq!(recs.len(), 6);
        assert!(recs
            .iter()
            .any(|r| r.fields.get("氏名").and_then(|v| v.as_str()) == Some("田中太郎")));
    }

    #[test]
    fn column_order_reorders_fields_and_appends_unlisted() {
        let recs = fetch(
            "csv",
            serde_json::json!({
                "path": sample("customers.csv"),
                "column_order": ["拠点", "氏名"],
            }),
        );
        let keys: Vec<&String> = recs[0].fields.keys().collect();
        // 指定した2列がその順で先頭に来て、指定しなかった列は元の並びのまま末尾に続く。
        assert_eq!(keys[0], "拠点");
        assert_eq!(keys[1], "氏名");
        assert!(keys.len() > 2);
    }

    #[test]
    fn display_columns_hides_fields_but_not_search_or_label() {
        let recs = fetch(
            "csv",
            serde_json::json!({
                "path": sample("customers.csv"),
                "display_columns": ["氏名"],
                "label_column": "拠点",
            }),
        );
        let r = &recs[0];
        // 表示は「氏名」だけに絞られる。
        assert_eq!(r.fields.len(), 1);
        assert!(r.fields.contains_key("氏名"));
        // label は display_columns の影響を受けず label_column から取れる。
        assert!(r.label.is_some());
        // 検索テキストは display で除外した列も含む(全フィールドから計算)。
        assert!(r
            .search_text
            .contains(r.fields.get("氏名").unwrap().as_str().unwrap()));
    }

    #[test]
    fn column_aliases_rename_display_key() {
        let recs = fetch(
            "csv",
            serde_json::json!({ "path": sample("customers.csv"), "column_aliases": { "氏名": "Name" } }),
        );
        let r = &recs[0];
        assert!(r.fields.contains_key("Name"));
        assert!(!r.fields.contains_key("氏名"));
    }

    #[test]
    fn strip_path_quotes_removes_surrounding_quotes() {
        assert_eq!(strip_path_quotes("\"C:\\a b\\x.csv\""), "C:\\a b\\x.csv");
        assert_eq!(strip_path_quotes("C:\\a\\x.csv"), "C:\\a\\x.csv");
        assert_eq!(strip_path_quotes("  \"C:\\q.csv\"  "), "C:\\q.csv");
        assert_eq!(strip_path_quotes("\""), "\""); // 1 文字だけならそのまま
    }

    #[test]
    fn quoted_path_is_read_via_build_connector() {
        // 両端クォート付きパスでも CSV が読める。
        let quoted = format!("\"{}\"", sample("customers.csv"));
        let recs = fetch("csv", serde_json::json!({ "path": quoted }));
        assert_eq!(recs.len(), 8);
    }

    #[test]
    fn webkit_epoch_conversion_matches_known_value() {
        // 2021-01-01 00:00:00 UTC の WebKit(1601 起点マイクロ秒)値を Unix 秒へ変換。
        let webkit_us: i64 = 13_253_932_800_000_000;
        let unix_secs = webkit_micros_to_unix_secs(webkit_us);
        assert_eq!(fmt_epoch(unix_secs), "2021-01-01 00:00:00");
    }

    #[test]
    fn chromium_bookmarks_json_parses_nested_folders() {
        let json = serde_json::json!({
            "roots": {
                "bookmark_bar": {
                    "type": "folder",
                    "name": "ブックマーク バー",
                    "children": [
                        { "type": "url", "name": "Example", "url": "https://example.com", "date_added": "13242816000000000" },
                        {
                            "type": "folder", "name": "サブフォルダ",
                            "children": [
                                { "type": "url", "name": "Nested", "url": "https://nested.example.com", "date_added": "13242816000000000" }
                            ]
                        }
                    ]
                },
                "other": { "type": "folder", "name": "その他", "children": [] }
            }
        });
        let tmp = std::env::temp_dir().join("everydb_test_bookmarks.json");
        std::fs::write(&tmp, json.to_string()).unwrap();
        let recs = fetch(
            "browser_bookmarks",
            serde_json::json!({ "browser": "chrome", "path": tmp.to_string_lossy() }),
        );
        std::fs::remove_file(&tmp).ok();

        assert_eq!(recs.len(), 2);
        let top = recs
            .iter()
            .find(|r| r.open_ref.as_deref() == Some("https://example.com"))
            .unwrap();
        assert_eq!(
            top.fields.get("フォルダ").and_then(|v| v.as_str()),
            Some("ブックマーク バー")
        );
        let nested = recs
            .iter()
            .find(|r| r.open_ref.as_deref() == Some("https://nested.example.com"))
            .unwrap();
        assert_eq!(
            nested.fields.get("フォルダ").and_then(|v| v.as_str()),
            Some("ブックマーク バー/サブフォルダ")
        );
    }

    // 実機の Chrome/Edge プロファイルが必要なライブ検証。
    // 実行: cargo test -- --ignored browser_live
    #[test]
    #[ignore = "requires a real Chrome/Edge profile on this machine"]
    fn browser_live_history_and_bookmarks() {
        let local = std::env::var("LOCALAPPDATA").expect("LOCALAPPDATA");
        let hist = fetch(
            "browser_history",
            serde_json::json!({
                "browser": "chrome", "path": format!("{local}\\Google\\Chrome\\User Data\\Default\\History")
            }),
        );
        println!("chrome history = {}", hist.len());
        assert!(!hist.is_empty());

        let bm = fetch(
            "browser_bookmarks",
            serde_json::json!({
                "browser": "edge", "path": format!("{local}\\Microsoft\\Edge\\User Data\\Default\\Bookmarks")
            }),
        );
        println!("edge bookmarks = {}", bm.len());
        assert!(!bm.is_empty());
    }

    // 実行: cargo test -- --ignored browser_live
    #[test]
    #[ignore = "requires a real Chrome/Edge profile on this machine with download history"]
    fn browser_live_downloads() {
        let local = std::env::var("LOCALAPPDATA").expect("LOCALAPPDATA");
        let dl = fetch(
            "browser_downloads",
            serde_json::json!({
                "browser": "chrome", "path": format!("{local}\\Google\\Chrome\\User Data\\Default\\History")
            }),
        );
        println!("chrome downloads = {}", dl.len());
        assert!(!dl.is_empty());
    }

    #[test]
    fn basename_of_handles_windows_and_unix_separators() {
        assert_eq!(basename_of(r"C:\Users\a\Downloads\file.pdf"), "file.pdf");
        assert_eq!(basename_of("/home/a/file.pdf"), "file.pdf");
        assert_eq!(basename_of("file.pdf"), "file.pdf");
    }

    #[test]
    fn file_uri_to_path_decodes_percent_and_strips_scheme() {
        assert_eq!(
            file_uri_to_path("file:///C:/Users/a/My%20File.pdf"),
            r"C:\Users\a\My File.pdf"
        );
    }

    #[test]
    fn raw_field_names_ignores_role_config_and_lists_all_columns() {
        let cfg = serde_json::json!({
            "path": sample("customers.csv"),
            "display_columns": ["氏名"], // これが無視されて全列返るはず
        });
        let names = raw_field_names("csv", &cfg.to_string()).unwrap();
        assert!(names.contains(&"氏名".to_string()));
        assert!(names.contains(&"部署".to_string()));
        assert!(names.contains(&"メール".to_string()));
    }

    #[test]
    fn fixed_schema_local_connectors_list_fields_without_reading_data() {
        // path は意図的に存在しない。列名プレビューで実データの読み込みを行わないことを固定する。
        let missing_path = "C:/not-a-real-path/for-field-preview";
        let cases: [(&str, serde_json::Value, &[&str]); 7] = [
            (
                "folder",
                serde_json::json!({ "path": missing_path }),
                &["名前", "パス", "拡張子", "サイズ", "更新日時"],
            ),
            (
                "ime_msime",
                serde_json::json!({ "path": missing_path }),
                &["読み", "語", "品詞"],
            ),
            (
                "ime_google",
                serde_json::json!({ "path": missing_path }),
                &["読み", "単語", "品詞", "コメント"],
            ),
            (
                "browser_history",
                serde_json::json!({ "browser": "chrome", "path": missing_path }),
                &["タイトル", "URL", "訪問回数", "最終訪問"],
            ),
            (
                "browser_bookmarks",
                serde_json::json!({ "browser": "firefox", "path": missing_path }),
                &["名前", "URL", "追加日時"],
            ),
            (
                "browser_downloads",
                serde_json::json!({ "browser": "chrome", "path": missing_path }),
                &[
                    "ファイル名",
                    "保存先パス",
                    "URL",
                    "状態",
                    "サイズ(バイト)",
                    "開始日時",
                    "完了日時",
                ],
            ),
            (
                "excel_sheets",
                serde_json::json!({ "path": missing_path }),
                &["シート名"],
            ),
        ];

        for (type_, config, expected) in cases {
            assert_eq!(
                raw_field_names(type_, &config.to_string()).unwrap(),
                expected
                    .iter()
                    .map(|name| (*name).to_string())
                    .collect::<Vec<_>>(),
                "{type_}"
            );
        }
    }
    #[test]
    fn excel_range_single_cell() {
        // 単一セル "B2"(has_header 無効相当)は 1 レコード・列文字 B を見出しに。
        let recs = fetch(
            "excel_range",
            serde_json::json!({ "path": sample("employees.xlsx"), "sheet": "設定", "range": "B2" }),
        );
        assert_eq!(recs.len(), 1);
        assert_eq!(
            recs[0].fields.get("B").and_then(|v| v.as_str()),
            Some("サンプル株式会社")
        );
    }

    #[test]
    fn excel_range_multi_row_uses_first_row_as_header() {
        // 社員シートの A1:C4 → 先頭行(社員番号/氏名/部署)をヘッダーに、3 レコード。
        let recs = fetch(
            "excel_range",
            serde_json::json!({ "path": sample("employees.xlsx"), "sheet": "社員", "range": "A1:C4" }),
        );
        assert_eq!(recs.len(), 3);
        assert!(recs
            .iter()
            .any(|r| r.fields.get("氏名").and_then(|v| v.as_str()) == Some("田中太郎")));
        assert!(recs[0].fields.contains_key("社員番号"));
    }

    #[test]
    fn excel_cell_legacy_type_still_works() {
        // 旧 "excel_cell" 種別 + "cell" フィールドも後方互換で動く。
        let recs = fetch(
            "excel_cell",
            serde_json::json!({ "path": sample("employees.xlsx"), "sheet": "設定", "cell": "B2" }),
        );
        assert_eq!(recs.len(), 1);
        assert_eq!(
            recs[0].fields.get("B").and_then(|v| v.as_str()),
            Some("サンプル株式会社")
        );
    }

    #[test]
    fn parse_ref_columns_works() {
        assert_eq!(parse_ref_columns("A, C:E, B").unwrap(), vec![0, 1, 2, 3, 4]);
        assert_eq!(parse_ref_columns("B:B, D").unwrap(), vec![1, 3]);
    }

    #[test]
    fn excel_sheets_lists_names() {
        let recs = fetch(
            "excel_sheets",
            serde_json::json!({ "path": sample("employees.xlsx") }),
        );
        let names: Vec<&str> = recs
            .iter()
            .filter_map(|r| r.fields.get("シート名").and_then(|v| v.as_str()))
            .collect();
        assert_eq!(names, vec!["社員", "テーブルサンプル", "拠点", "設定"]);
    }

    #[test]
    fn folder_recursive_vs_flat() {
        let all = fetch(
            "folder",
            serde_json::json!({ "path": sample("tree"), "recursive": true }),
        );
        assert_eq!(all.len(), 6);
        let flat = fetch(
            "folder",
            serde_json::json!({ "path": sample("tree"), "recursive": false }),
        );
        assert_eq!(flat.len(), 1);
    }

    #[test]
    fn folder_extension_filter() {
        let csvs = fetch(
            "folder",
            serde_json::json!({ "path": sample("tree"), "recursive": true, "extensions": ["csv"] }),
        );
        assert_eq!(csvs.len(), 2);
    }

    #[test]
    fn ime_msime_utf16_skips_comment() {
        let recs = fetch(
            "ime_msime",
            serde_json::json!({ "path": sample("msime_dict.txt") }),
        );
        assert_eq!(recs.len(), 5); // 先頭の ! コメント行は除外
        assert!(recs
            .iter()
            .any(|r| r.fields.get("語").and_then(|v| v.as_str()) == Some("東京")));
        // label は 2 列目(語)
        assert_eq!(recs[0].label.as_deref(), Some("東京"));
    }

    #[test]
    fn ime_google_utf8() {
        let recs = fetch(
            "ime_google",
            serde_json::json!({ "path": sample("google_dict.txt") }),
        );
        assert_eq!(recs.len(), 4);
        assert!(recs
            .iter()
            .any(|r| r.fields.get("単語").and_then(|v| v.as_str()) == Some("EveryDB")));
    }

    // Access ドライバ(ACE ODBC)がインストールされた環境でのみ成功する。
    // 実行: cargo test -- --ignored  （通常の cargo test では skip）
    #[test]
    #[ignore = "requires Microsoft Access ODBC driver + samples/access_db.accdb"]
    fn accdb_reads_japanese_rows() {
        let recs = fetch(
            "accdb",
            serde_json::json!({ "path": sample("access_db.accdb"), "table": "取引先" }),
        );
        assert_eq!(recs.len(), 5);
        assert!(recs.iter().any(
            |r| r.fields.get("会社名").and_then(|v| v.as_str()) == Some("株式会社サンプル商事")
        ));
        assert!(recs
            .iter()
            .any(|r| r.fields.get("担当者").and_then(|v| v.as_str()) == Some("渡辺健")));
    }

    // --- Notion: プロパティ変換のオフライン検証（ネットワーク不要）---
    #[test]
    fn notion_prop_conversions() {
        use serde_json::json;
        let title = json!({ "type": "title", "title": [{ "plain_text": "案件A" }] });
        assert_eq!(notion_prop_to_string(&title), "案件A");

        let sel = json!({ "type": "select", "select": { "name": "進行中" } });
        assert_eq!(notion_prop_to_string(&sel), "進行中");

        let multi = json!({ "type": "multi_select", "multi_select": [{ "name": "緊急" }, { "name": "営業" }] });
        assert_eq!(notion_prop_to_string(&multi), "緊急, 営業");

        let num = json!({ "type": "number", "number": 42 });
        assert_eq!(notion_prop_to_string(&num), "42");

        let chk = json!({ "type": "checkbox", "checkbox": true });
        assert_eq!(notion_prop_to_string(&chk), "✓");

        let date = json!({ "type": "date", "date": { "start": "2026-07-24", "end": null } });
        assert_eq!(notion_prop_to_string(&date), "2026-07-24");

        let range =
            json!({ "type": "date", "date": { "start": "2026-07-01", "end": "2026-07-31" } });
        assert_eq!(notion_prop_to_string(&range), "2026-07-01 ~ 2026-07-31");

        let empty_sel = json!({ "type": "select", "select": null });
        assert_eq!(notion_prop_to_string(&empty_sel), "");
    }

    #[test]
    fn graph_flat_record_maps_user() {
        use serde_json::json;
        let user = json!({
            "displayName": "山田太郎",
            "userPrincipalName": "yamada@example.onmicrosoft.com",
            "mail": "yamada@example.com",
            "jobTitle": "課長",
            "department": "営業部",
            "id": "guid-123"
        });
        let fields = [
            "displayName",
            "userPrincipalName",
            "mail",
            "jobTitle",
            "department",
        ];
        let cfg = CommonCfg::default();
        let rec = graph_flat_record(&user, &fields, "displayName", mailto_open_ref(&user), &cfg);
        assert_eq!(rec.label.as_deref(), Some("山田太郎"));
        assert_eq!(rec.open_ref.as_deref(), Some("mailto:yamada@example.com"));
        assert_eq!(
            rec.fields.get("department").and_then(|v| v.as_str()),
            Some("営業部")
        );
        assert!(rec.search_text.contains("課長"));
    }

    #[test]
    fn graph_scalar_handles_array_and_missing() {
        use serde_json::json;
        assert_eq!(
            json_scalar_to_string(&json!(["03-1", "080-2"])),
            "03-1, 080-2"
        );
        assert_eq!(json_scalar_to_string(&json!(null)), "");
        assert_eq!(json_scalar_to_string(&json!(42)), "42");
        // mail 欠如なら open_ref は None
        assert_eq!(mailto_open_ref(&json!({ "displayName": "x" })), None);
    }

    #[test]
    fn notion_page_normalization_picks_title_as_label() {
        use serde_json::json;
        let page = json!({
            "url": "https://www.notion.so/xyz",
            "properties": {
                "ステータス": { "type": "select", "select": { "name": "完了" } },
                "名称": { "type": "title", "title": [{ "plain_text": "サンプル案件" }] }
            }
        });
        let cfg = CommonCfg::default();
        let rec = notion_page_to_record(&page, &cfg);
        assert_eq!(rec.label.as_deref(), Some("サンプル案件")); // title 型を label に採用
        assert_eq!(rec.open_ref.as_deref(), Some("https://www.notion.so/xyz"));
        assert!(rec.search_text.contains("完了"));
    }

    // ライブ結合テスト。DB をインテグレーションに共有し、環境変数を設定して実行:
    //   $env:EVERYDB_NOTION_TOKEN='...'; $env:EVERYDB_NOTION_DB='...'; cargo test -- --ignored notion_live
    #[test]
    #[ignore = "requires EVERYDB_NOTION_TOKEN + EVERYDB_NOTION_DB env vars and a shared DB"]
    fn notion_live_reads_database() {
        let token = std::env::var("EVERYDB_NOTION_TOKEN").expect("EVERYDB_NOTION_TOKEN 未設定");
        let db = std::env::var("EVERYDB_NOTION_DB").expect("EVERYDB_NOTION_DB 未設定");
        let recs = fetch(
            "notion",
            serde_json::json!({ "token": token, "database_id": db }),
        );
        println!("Notion から {} 件取得", recs.len());
        assert!(!recs.is_empty(), "1 件以上取得できること");
        // 各レコードは open_ref(ページ URL) を持つ。
        assert!(recs.iter().all(|r| r
            .open_ref
            .as_deref()
            .map(|u| u.starts_with("http"))
            .unwrap_or(false)));
    }

    // ライブ結合テスト。サービスアカウントの鍵ファイルを共有し、環境変数を設定して実行:
    //   $env:EVERYDB_GOOGLE_KEY_PATH='...'; $env:EVERYDB_GSHEET_ID='...'; cargo test -- --ignored gsheets_live
    #[test]
    #[ignore = "requires EVERYDB_GOOGLE_KEY_PATH + EVERYDB_GSHEET_ID env vars and a shared sheet"]
    fn gsheets_live_reads_rows() {
        let key_path =
            std::env::var("EVERYDB_GOOGLE_KEY_PATH").expect("EVERYDB_GOOGLE_KEY_PATH 未設定");
        let spreadsheet_id = std::env::var("EVERYDB_GSHEET_ID").expect("EVERYDB_GSHEET_ID 未設定");
        let recs = fetch(
            "google_sheets",
            serde_json::json!({ "key_path": key_path, "spreadsheet_id": spreadsheet_id }),
        );
        println!("Google Sheets から {} 件取得", recs.len());
        assert!(!recs.is_empty());
    }

    #[test]
    fn ical_unfold_joins_continuation_lines() {
        let raw = "BEGIN:VEVENT\nSUMMARY:foo\n bar\nEND:VEVENT";
        assert_eq!(ical_unfold(raw), "BEGIN:VEVENT\nSUMMARY:foobar\nEND:VEVENT");
    }

    #[test]
    fn ical_datetime_and_unescape() {
        assert_eq!(
            parse_ical_datetime("20260101T090000Z"),
            "2026-01-01 09:00:00"
        );
        assert_eq!(parse_ical_datetime("20260101"), "2026-01-01");
        assert_eq!(
            ical_unescape(r"line1\nline2\, comma"),
            "line1\nline2, comma"
        );
    }

    // ライブ結合テスト: 公開 .ics URL を用意して実行:
    //   $env:EVERYDB_ICAL_URL='https://.../basic.ics'; cargo test -- --ignored ical_live
    #[test]
    #[ignore = "requires EVERYDB_ICAL_URL env var (public .ics)"]
    fn ical_live_reads_events() {
        let url = std::env::var("EVERYDB_ICAL_URL").expect("EVERYDB_ICAL_URL 未設定");
        let recs = fetch("ical_calendar", serde_json::json!({ "url": url }));
        println!("iCal から {} 件取得", recs.len());
        assert!(!recs.is_empty());
    }

    #[test]
    fn airtable_value_flattens_arrays_and_attachments() {
        use serde_json::json;
        assert_eq!(airtable_value_to_string(&json!("plain")), "plain");
        assert_eq!(airtable_value_to_string(&json!(["a", "b"])), "a, b");
        assert_eq!(
            airtable_value_to_string(&json!([{ "filename": "x.png" }, { "filename": "y.png" }])),
            "x.png, y.png"
        );
    }

    // ライブ結合テスト:
    //   $env:EVERYDB_AIRTABLE_TOKEN='...'; $env:EVERYDB_AIRTABLE_BASE='...'; $env:EVERYDB_AIRTABLE_TABLE='...';
    //   cargo test -- --ignored airtable_live
    #[test]
    #[ignore = "requires EVERYDB_AIRTABLE_TOKEN/_BASE/_TABLE env vars"]
    fn airtable_live_reads_records() {
        let token = std::env::var("EVERYDB_AIRTABLE_TOKEN").expect("EVERYDB_AIRTABLE_TOKEN 未設定");
        let base_id = std::env::var("EVERYDB_AIRTABLE_BASE").expect("EVERYDB_AIRTABLE_BASE 未設定");
        let table = std::env::var("EVERYDB_AIRTABLE_TABLE").expect("EVERYDB_AIRTABLE_TABLE 未設定");
        let recs = fetch(
            "airtable",
            serde_json::json!({ "token": token, "base_id": base_id, "table": table }),
        );
        println!("Airtable から {} 件取得", recs.len());
        assert!(!recs.is_empty());
    }
}

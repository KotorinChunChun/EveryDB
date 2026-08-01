use anyhow::{anyhow, Result};
use rusqlite::{Connection, OpenFlags};
use serde::Deserialize;
use serde_json::Value;
use std::io::Read;

use crate::graph::GraphClient;
use crate::model::Record;

mod common;
use common::{make_record, make_record_with_row, CommonCfg};
mod local;
use local::{
    AccessConnector, CsvConnector, FolderConnector, ImeCfg, ImeConnector, ImeFlavor,
    SqliteConnector,
};
mod database;
mod public_data;
use database::{MysqlConnector, PostgresConnector};

use public_data::{
    CorporateNumberConnector, PokeApiConnector, PostalCodeConnector, WebPageConnector,
};

pub(crate) fn default_true() -> bool {
    true
}

/// データソース取得の共通拡張点（要件 §5-3）。
pub trait Connector {
    fn fetch(&self) -> Result<Vec<Record>>;
    /// レコード全件を取得せず、フィールド（列）名の一覧のみを高速に取得する。
    fn field_names(&self) -> Result<Vec<String>> {
        let records = self.fetch()?;
        let mut seen = std::collections::HashSet::new();
        let mut names = Vec::new();
        for r in &records {
            for k in r.fields.keys() {
                if seen.insert(k.clone()) {
                    names.push(k.clone());
                }
            }
        }
        Ok(names)
    }
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

/// config_json 内のパス系フィールド値の両端クォートを正規化する。
fn normalize_path_in_config(config_json: &str) -> Result<String> {
    let mut v: Value = serde_json::from_str(config_json)?;
    let mut changed = false;
    if let Some(obj) = v.as_object_mut() {
        for key in ["path", "key_path"] {
            if let Some(p) = obj.get(key).and_then(|x| x.as_str()) {
                let cleaned = strip_path_quotes(p);
                obj.insert(key.to_string(), Value::String(cleaned));
                changed = true;
            }
        }
    }
    if changed {
        Ok(serde_json::to_string(&v)?)
    } else {
        Ok(config_json.to_string())
    }
}

/// Rust factoryが受け入れる安定したデータソースID。フロントのmetadataと対応を保つ。
pub const SUPPORTED_SOURCE_TYPES: &[&str] = &[
    "csv",
    "sqlite",
    "accdb",
    "excel_table",
    "excel_range",
    "excel_cell",
    "excel_sheets",
    "folder",
    "ime_msime",
    "ime_google",
    "notion",
    "graph_entra_users",
    "graph_entra_groups",
    "graph_teams",
    "graph_sharepoint_list",
    "graph_sharepoint_library",
    "graph_entra_devices",
    "graph_intune_devices",
    "graph_licenses",
    "graph_calendar",
    "graph_onedrive",
    "graph_planner",
    "browser_history",
    "browser_bookmarks",
    "browser_downloads",
    "google_sheets",
    "ical_calendar",
    "airtable",
    "postgres",
    "mysql",
    "box_files",
    "postal_code",
    "corporate_number",
    "web_page",
    "pokeapi",
];

pub fn is_supported_source_type(type_: &str) -> bool {
    SUPPORTED_SOURCE_TYPES.contains(&type_)
}
/// 種別 + config_json からコネクタを生成するファクトリ。
pub fn build_connector(type_: &str, config_json: &str) -> Result<Box<dyn Connector>> {
    use serde_json::from_str;
    if !is_supported_source_type(type_) {
        return Err(anyhow!("未対応のデータソース種別: {type_}"));
    }
    // 保存時に暗号化された秘密フィールドをここで復号する（要件 §4-3-bis）。
    let config_json = crate::secret::unprotect_config(config_json)?;
    // ファイルパスの両端クォートを除去する（要件: クォート付きパス対応）。
    let config_json = normalize_path_in_config(&config_json)?;
    let config_json = config_json.as_str();
    match type_ {
        "csv" => Ok(Box::new(from_str::<CsvConnector>(config_json)?)),
        "sqlite" => Ok(Box::new(from_str::<SqliteConnector>(config_json)?)),
        "excel_table" => Ok(Box::new(from_str::<ExcelTableConnector>(config_json)?)),
        "excel_range" | "excel_cell" => Ok(Box::new(from_str::<ExcelRangeConnector>(config_json)?)),
        "excel_sheets" => Ok(Box::new(from_str::<ExcelSheetsConnector>(config_json)?)),
        "folder" => Ok(Box::new(from_str::<FolderConnector>(config_json)?)),
        "ime_msime" => Ok(Box::new(ImeConnector::new(
            from_str::<ImeCfg>(config_json)?,
            ImeFlavor::MsIme,
        ))),
        "ime_google" => Ok(Box::new(ImeConnector::new(
            from_str::<ImeCfg>(config_json)?,
            ImeFlavor::Google,
        ))),
        "accdb" => Ok(Box::new(from_str::<AccessConnector>(config_json)?)),
        "notion" => Ok(Box::new(from_str::<NotionConnector>(config_json)?)),
        "graph_entra_users" => Ok(Box::new(from_str::<GraphUsersConnector>(config_json)?)),
        "graph_entra_groups" => Ok(Box::new(from_str::<GraphGroupsConnector>(config_json)?)),
        "graph_teams" => Ok(Box::new(from_str::<GraphTeamsConnector>(config_json)?)),
        "graph_sharepoint_list" => Ok(Box::new(from_str::<GraphSharePointListConnector>(
            config_json,
        )?)),
        "graph_sharepoint_library" => Ok(Box::new(from_str::<GraphSharePointLibraryConnector>(
            config_json,
        )?)),
        "browser_history" => Ok(Box::new(from_str::<BrowserHistoryConnector>(config_json)?)),
        "browser_bookmarks" => Ok(Box::new(from_str::<BrowserBookmarksConnector>(
            config_json,
        )?)),
        "browser_downloads" => Ok(Box::new(from_str::<BrowserDownloadsConnector>(
            config_json,
        )?)),
        "graph_entra_devices" => Ok(Box::new(from_str::<GraphDevicesConnector>(config_json)?)),
        "graph_intune_devices" => Ok(Box::new(from_str::<GraphIntuneDevicesConnector>(
            config_json,
        )?)),
        "graph_licenses" => Ok(Box::new(from_str::<GraphLicensesConnector>(config_json)?)),
        "graph_calendar" => Ok(Box::new(from_str::<GraphCalendarConnector>(config_json)?)),
        "graph_onedrive" => Ok(Box::new(from_str::<GraphOneDriveConnector>(config_json)?)),
        "graph_planner" => Ok(Box::new(from_str::<GraphPlannerConnector>(config_json)?)),
        "google_sheets" => Ok(Box::new(from_str::<GoogleSheetsConnector>(config_json)?)),
        "ical_calendar" => Ok(Box::new(from_str::<IcalConnector>(config_json)?)),
        "airtable" => Ok(Box::new(from_str::<AirtableConnector>(config_json)?)),
        "postgres" => Ok(Box::new(from_str::<PostgresConnector>(config_json)?)),
        "mysql" => Ok(Box::new(from_str::<MysqlConnector>(config_json)?)),
        "box_files" => Ok(Box::new(from_str::<BoxConnector>(config_json)?)),
        "postal_code" => Ok(Box::new(from_str::<PostalCodeConnector>(config_json)?)),
        "corporate_number" => Ok(Box::new(from_str::<CorporateNumberConnector>(config_json)?)),
        "web_page" => Ok(Box::new(from_str::<WebPageConnector>(config_json)?)),
        "pokeapi" => Ok(Box::new(from_str::<PokeApiConnector>(config_json)?)),
        other => Err(anyhow!(
            "factory定義が不足しているデータソース種別: {other}"
        )),
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

// ---- Excel 共通 ----
pub(crate) fn cell_to_string(d: &calamine::Data) -> String {
    use calamine::Data as D;
    match d {
        D::Empty => String::new(),
        D::String(s) => s.clone(),
        D::Float(f) => {
            if f.fract() == 0.0 && f.abs() < 1e15 {
                format!("{}", *f as i64)
            } else {
                f.to_string()
            }
        }
        D::Int(i) => i.to_string(),
        D::Bool(b) => b.to_string(),
        D::DateTime(dt) => dt.to_string(),
        D::DateTimeIso(s) => s.clone(),
        D::DurationIso(s) => s.clone(),
        D::Error(e) => format!("#{e:?}"),
    }
}

/// "B2" のような A1 表記を 0 始まりの (row, col) に変換。
fn parse_a1(cell: &str) -> Result<(u32, u32)> {
    let cell = cell.trim();
    let split = cell
        .find(|c: char| c.is_ascii_digit())
        .ok_or_else(|| anyhow!("不正なセル指定: {cell}"))?;
    let (letters, digits) = cell.split_at(split);
    if letters.is_empty() || digits.is_empty() {
        return Err(anyhow!("不正なセル指定: {cell}"));
    }
    let mut col: u32 = 0;
    for ch in letters.chars() {
        let up = ch.to_ascii_uppercase();
        if !up.is_ascii_alphabetic() {
            return Err(anyhow!("不正な列: {cell}"));
        }
        col = col * 26 + (up as u32 - 'A' as u32 + 1);
    }
    let row: u32 = digits.parse().map_err(|_| anyhow!("不正な行: {cell}"))?;
    if col == 0 || row == 0 {
        return Err(anyhow!("不正なセル指定: {cell}"));
    }
    Ok((row - 1, col - 1))
}

// ---- Excel: テーブル（ListObject）を読む ----
// 「Excel のテーブル」とは Excel 上で [挿入]→[テーブル] で定義した構造化テーブル(ListObject)を指す。
// 旧実装はシート全体を「テーブル」と称して読んでいたが、これは実際にはただの使用範囲読み取りであり
// ListObject ではなかった（ユーザー指摘により修正）。calamine 0.26 の Xlsx::load_tables/table_by_name
// で真の ListObject を読み、旧データ（table キー未設定・sheet キーのみ）はシート全体読みへ後方互換で
// フォールバックする。
#[derive(Deserialize)]
struct ExcelTableConnector {
    path: String,
    /// テーブル(ListObject)名。新規追加時はこちらを使う。
    #[serde(default)]
    table: Option<String>,
    /// 旧仕様: シート名（ListObject 導入前のデータとの後方互換用）。シート全体を読む。
    #[serde(default)]
    sheet: Option<String>,
    #[serde(flatten)]
    common: CommonCfg,
}

impl Connector for ExcelTableConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        use calamine::Reader;
        if let Some(table_name) = self.table.as_deref().filter(|s| !s.is_empty()) {
            let mut wb: calamine::Xlsx<_> = calamine::open_workbook(&self.path).map_err(|e| {
                anyhow!("Excel テーブル(ListObject)の読み込みには xlsx/xlsm 形式が必要です: {e}")
            })?;
            wb.load_tables()?;
            let table = wb.table_by_name(table_name)?;
            let headers = table.columns().to_vec();
            let base_row = (table.data().start().map(|(r, _)| r).unwrap_or(0) + 2) as i64; // ヘッダーの次(データ1行目)の絶対物理行番号 (1-indexed)
            let mut out = Vec::new();
            for (idx, row) in table.data().rows().enumerate() {
                let pairs: Vec<(String, String)> = (0..headers.len())
                    .map(|i| {
                        let name = if headers[i].is_empty() {
                            format!("col{}", i + 1)
                        } else {
                            headers[i].clone()
                        };
                        let val = row.get(i).map(cell_to_string).unwrap_or_default();
                        (name, val)
                    })
                    .collect();
                if pairs.iter().all(|(_, v)| v.is_empty()) {
                    continue;
                }
                let row_num = base_row + (idx as i64);
                out.push(make_record_with_row(
                    pairs,
                    &self.path,
                    &self.common,
                    Some(row_num),
                ));
            }
            return Ok(out);
        }

        let sheet = self
            .sheet
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("テーブル(ListObject)名が指定されていません"))?;
        let mut wb = calamine::open_workbook_auto(&self.path)?;
        let range = wb.worksheet_range(sheet)?;
        let mut iter = range.rows();
        let headers: Vec<String> = match iter.next() {
            Some(first) => first.iter().map(cell_to_string).collect(),
            None => return Ok(Vec::new()),
        };
        let mut out = Vec::new();
        for row in iter {
            if row.iter().all(|c| matches!(c, calamine::Data::Empty)) {
                continue;
            }
            let pairs: Vec<(String, String)> = (0..headers.len())
                .map(|i| {
                    let name = if headers[i].is_empty() {
                        format!("col{}", i + 1)
                    } else {
                        headers[i].clone()
                    };
                    let val = row.get(i).map(cell_to_string).unwrap_or_default();
                    (name, val)
                })
                .collect();
            out.push(make_record(pairs, &self.path, &self.common));
        }
        Ok(out)
    }

    fn field_names(&self) -> Result<Vec<String>> {
        use calamine::Reader;
        if let Some(table_name) = self.table.as_deref().filter(|s| !s.is_empty()) {
            let mut wb: calamine::Xlsx<_> = calamine::open_workbook(&self.path).map_err(|e| {
                anyhow!("Excel テーブル(ListObject)の読み込みには xlsx/xlsm 形式が必要です: {e}")
            })?;
            wb.load_tables()?;
            let table = wb.table_by_name(table_name)?;
            return Ok(table.columns().to_vec());
        }
        let sheet = self
            .sheet
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("テーブル(ListObject)名が指定されていません"))?;
        let mut wb = calamine::open_workbook_auto(&self.path)?;
        let range = wb.worksheet_range(sheet)?;
        if let Some(first) = range.rows().next() {
            Ok(first.iter().map(cell_to_string).collect())
        } else {
            Ok(Vec::new())
        }
    }
}

/// 0 始まり列番号(0=A, 1=B, ...)を Excel の列文字に変換する。
fn col_to_letters(mut col: u32) -> String {
    let mut s = Vec::new();
    loop {
        s.push((b'A' + (col % 26) as u8) as char);
        if col < 26 {
            break;
        }
        col = col / 26 - 1;
    }
    s.iter().rev().collect()
}

/// "A1:C10" のような範囲を ((r1,c1),(r2,c2)) の 0 始まり座標に変換する。単一セル "A1" も許容。
fn parse_a1_range(range: &str) -> Result<((u32, u32), (u32, u32))> {
    let range = range.trim();
    if let Some((a, b)) = range.split_once(':') {
        let (r1, c1) = parse_a1(a)?;
        let (r2, c2) = parse_a1(b)?;
        Ok(((r1.min(r2), c1.min(c2)), (r1.max(r2), c1.max(c2))))
    } else {
        let (r, c) = parse_a1(range)?;
        Ok(((r, c), (r, c)))
    }
}

// ---- Excel: セル範囲 ----

/// xlsx ファイルからオートフィルタの範囲（`<autoFilter ref="A1:D100"/>` の ref 属性）を読み取る。
/// calamine では取得できないため、zip でシートの XML を直接パースする。
fn read_autofilter_ref(path: &str, sheet_name: &str) -> Option<((u32, u32), (u32, u32))> {
    let file = std::fs::File::open(path).ok()?;
    let mut zip = zip::ZipArchive::new(file).ok()?;

    // workbook.xml からシート名→sheetN.xml のマッピングを取得する。
    // まず xl/workbook.xml を読んでシート名と rId の対応を取得し、
    // xl/_rels/workbook.xml.rels から rId→ファイルパスの対応を取得する。
    let mut sheet_file = None;

    // workbook.xml から sheetId/rId を取得
    let mut rid: Option<String> = None;
    if let Ok(mut wbf) = zip.by_name("xl/workbook.xml") {
        let mut xml = String::new();
        wbf.read_to_string(&mut xml).ok()?;
        // 簡易パース: <sheet name="Sheet1" sheetId="1" r:id="rId1"/>
        for line in xml.split('<') {
            let line = line.trim();
            if !line.starts_with("sheet ") && !line.starts_with("sheet\t") {
                continue;
            }
            // name="..." を探す
            let name_val = extract_xml_attr(line, "name")?;
            if name_val == sheet_name {
                rid = extract_xml_attr(line, "r:id");
                break;
            }
        }
    }
    let rid = rid?;

    // workbook.xml.rels から rId → Target のマッピング
    if let Ok(mut relf) = zip.by_name("xl/_rels/workbook.xml.rels") {
        let mut xml = String::new();
        relf.read_to_string(&mut xml).ok()?;
        for line in xml.split('<') {
            let line = line.trim();
            if !line.starts_with("Relationship ") {
                continue;
            }
            if let Some(id) = extract_xml_attr(line, "Id") {
                if id == rid {
                    if let Some(target) = extract_xml_attr(line, "Target") {
                        // Target は "worksheets/sheet1.xml" のような相対パス
                        let full = if let Some(stripped) = target.strip_prefix('/') {
                            stripped.to_string()
                        } else {
                            format!("xl/{target}")
                        };
                        sheet_file = Some(full);
                    }
                    break;
                }
            }
        }
    }

    let sheet_file = sheet_file?;

    // シートの XML から <autoFilter ref="A1:D100"/> を探す
    if let Ok(mut sf) = zip.by_name(&sheet_file) {
        let mut xml = String::new();
        sf.read_to_string(&mut xml).ok()?;
        for line in xml.split('<') {
            let line = line.trim();
            if line.starts_with("autoFilter ") || line.starts_with("autoFilter\t") {
                if let Some(ref_val) = extract_xml_attr(line, "ref") {
                    return parse_a1_range(&ref_val).ok();
                }
            }
        }
    }

    None
}

/// XML 要素の属性値を簡易的に取り出すヘルパー（例: `name="Sheet1"` → `Some("Sheet1")`）。
fn extract_xml_attr(element: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=\"");
    let start = element.find(&needle)? + needle.len();
    let rest = &element[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn is_cell_non_empty(v: &calamine::Data) -> bool {
    match v {
        calamine::Data::Empty => false,
        calamine::Data::String(s) => !s.trim().is_empty(),
        _ => true,
    }
}

/// calamine::Range からデータが存在する先頭行（行, 列）を特定する。
/// `ref_cols` が指定されている場合はそのいずれかの列のみを調べ、なければ全列を走査する。
fn detect_start_cell(
    range: &calamine::Range<calamine::Data>,
    ref_cols: &[u32],
) -> Option<(u32, u32)> {
    let (start, end) = (
        range.start().unwrap_or((0, 0)),
        range.end().unwrap_or((0, 0)),
    );

    if !ref_cols.is_empty() {
        for r in start.0..=end.0 {
            let has_val = ref_cols.iter().any(|&col| {
                range
                    .get_value((r, col))
                    .map(is_cell_non_empty)
                    .unwrap_or(false)
            });
            if has_val {
                let min_col = (start.1..=end.1)
                    .find(|&c| {
                        range
                            .get_value((r, c))
                            .map(is_cell_non_empty)
                            .unwrap_or(false)
                    })
                    .unwrap_or(start.1);
                return Some((r, min_col));
            }
        }
        None
    } else {
        for r in start.0..=end.0 {
            for c in start.1..=end.1 {
                if let Some(v) = range.get_value((r, c)) {
                    if is_cell_non_empty(v) {
                        return Some((r, c));
                    }
                }
            }
        }
        None
    }
}

/// calamine::Range からデータが存在する末尾行（行, 列）を特定する。
/// `ref_cols` が指定されている場合はそのいずれかの列のみを調べ、なければ全列を走査する。
fn detect_end_cell(
    range: &calamine::Range<calamine::Data>,
    ref_cols: &[u32],
) -> Option<(u32, u32)> {
    let (start, end) = (
        range.start().unwrap_or((0, 0)),
        range.end().unwrap_or((0, 0)),
    );

    if !ref_cols.is_empty() {
        // ref_cols 指定時: 下から上に走査し、いずれかの指定列に非空セルがある最初の行を探す
        for r in (start.0..=end.0).rev() {
            let has_val = ref_cols.iter().any(|&col| {
                range
                    .get_value((r, col))
                    .map(is_cell_non_empty)
                    .unwrap_or(false)
            });
            if has_val {
                return Some((r, end.1));
            }
        }
        None
    } else {
        // 全列走査: 下から上に走査し、非空セルが存在する最終行とその行/それまでの最右列を探す
        let mut max_c = start.1;
        let mut last_r = None;
        for r in (start.0..=end.0).rev() {
            let mut found_in_row = false;
            for c in start.1..=end.1 {
                if let Some(v) = range.get_value((r, c)) {
                    if is_cell_non_empty(v) {
                        found_in_row = true;
                        if c > max_c {
                            max_c = c;
                        }
                    }
                }
            }
            if found_in_row && last_r.is_none() {
                last_r = Some(r);
            }
        }
        last_r.map(|r| (r, max_c))
    }
}

/// "A, B" や "A:C" や "A, C:E" のような文字列を解析して列インデックス(0-indexed)の昇順ソート済みリスト(重複なし)を返す。
fn parse_ref_columns(s: &str) -> Result<Vec<u32>> {
    let mut cols = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((start_str, end_str)) = part.split_once(':') {
            let c1 = parse_col_letter(start_str.trim())?;
            let c2 = parse_col_letter(end_str.trim())?;
            let (min_c, max_c) = (c1.min(c2), c1.max(c2));
            for c in min_c..=max_c {
                cols.push(c);
            }
        } else {
            cols.push(parse_col_letter(part)?);
        }
    }
    cols.sort_unstable();
    cols.dedup();
    Ok(cols)
}

/// 列文字（"A", "AB" 等）を 0 始まりの列番号に変換する。
fn parse_col_letter(s: &str) -> Result<u32> {
    let s = s.trim().to_ascii_uppercase();
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(anyhow!("不正な列指定: {s}"));
    }
    let mut col: u32 = 0;
    for ch in s.chars() {
        col = col * 26 + (ch as u32 - 'A' as u32 + 1);
    }
    Ok(col - 1)
}

#[derive(Deserialize)]
struct ExcelRangeConnector {
    path: String,
    sheet: String,

    // ---- 新しいフィールド ----
    /// 先頭セルの取得方法: "auto" (既定) or "manual"
    #[serde(default = "default_auto")]
    start_mode: String,
    /// 手動指定時の先頭セル (A1 形式の単一セル)
    #[serde(default)]
    start_cell: Option<String>,
    /// 自動取得時の先頭セル用基準列 (任意。"A, B" や "A:C" 等)
    #[serde(default)]
    start_ref_column: Option<String>,
    /// 末尾セルの取得方法: "auto" (既定) or "manual"
    #[serde(default = "default_auto")]
    end_mode: String,
    /// 手動指定時の末尾セル (A1 形式の単一セル)
    #[serde(default)]
    end_cell: Option<String>,
    /// 自動取得時の末尾セル用基準列 (任意。"A, B" や "A:C" 等)
    #[serde(default)]
    end_ref_column: Option<String>,
    /// 旧・自動取得時の基準列 (任意。"A" のような列文字)
    #[serde(default)]
    ref_column: Option<String>,

    // ---- 旧フィールド（後方互換）----
    /// 旧 "A1:C10" 形式
    #[serde(default)]
    range: Option<String>,
    /// さらに旧い "cell" フィールド
    #[serde(default)]
    cell: Option<String>,

    /// 範囲の先頭行をヘッダーとして扱うか（既定 true）。false なら列文字を見出しにする。
    #[serde(default = "default_true")]
    has_header: bool,
    #[serde(flatten)]
    common: CommonCfg,
}

fn default_auto() -> String {
    "auto".to_string()
}

impl Connector for ExcelRangeConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        use calamine::Reader;

        // --- 後方互換: 旧 range/cell フィールドが設定されている場合はそちらを優先 ---
        if let Some(spec) = self
            .range
            .as_deref()
            .or(self.cell.as_deref())
            .filter(|s| !s.is_empty())
        {
            // 旧フォーマットのまま動作させる（start_mode/end_mode が既定 "auto" のまま来る場合もある）
            if self.start_cell.is_none() && self.end_cell.is_none() {
                let ((r1, c1), (r2, c2)) = parse_a1_range(spec)?;
                let mut wb = calamine::open_workbook_auto(&self.path)?;
                let range = wb.worksheet_range(&self.sheet)?;
                return self.read_range(&range, r1, c1, r2, c2);
            }
        }

        // --- 新しいフォーマット: start_mode / end_mode で先頭・末尾セルを決定 ---
        let mut wb = calamine::open_workbook_auto(&self.path)?;
        let range = wb.worksheet_range(&self.sheet)?;

        // ref_column を列番号リストへ変換
        let start_ref_str = self
            .start_ref_column
            .as_deref()
            .or(self.ref_column.as_deref());
        let start_ref_cols = match start_ref_str {
            Some(s) if !s.is_empty() => parse_ref_columns(s)?,
            _ => Vec::new(),
        };
        let end_ref_str = self
            .end_ref_column
            .as_deref()
            .or(self.ref_column.as_deref());
        let end_ref_cols = match end_ref_str {
            Some(s) if !s.is_empty() => parse_ref_columns(s)?,
            _ => Vec::new(),
        };

        // オートフィルタ情報を取得（xlsx のみ）
        let autofilter = read_autofilter_ref(&self.path, &self.sheet);

        // 先頭セルの決定
        let (r1, c1) = match self.start_mode.as_str() {
            "manual" => {
                let cell_str = self
                    .start_cell
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| anyhow!("先頭セルが手動指定ですが、値が入力されていません"))?;
                parse_a1(cell_str)?
            }
            _ => {
                // 自動取得: 優先順位
                // 1. オートフィルタの設定行（左上セル）
                // 2. ref_column で指定された列のデータ先頭行
                // 3. いずれかの列の先頭行
                if let Some(((ar, ac), _)) = autofilter {
                    (ar, ac)
                } else if !start_ref_cols.is_empty() {
                    detect_start_cell(&range, &start_ref_cols)
                        .or_else(|| detect_start_cell(&range, &[]))
                        .unwrap_or_else(|| range.start().unwrap_or((0, 0)))
                } else {
                    detect_start_cell(&range, &[])
                        .unwrap_or_else(|| range.start().unwrap_or((0, 0)))
                }
            }
        };

        // 末尾セルの決定
        let (r2, c2) = match self.end_mode.as_str() {
            "manual" => {
                let cell_str = self
                    .end_cell
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| anyhow!("末尾セルが手動指定ですが、値が入力されていません"))?;
                parse_a1(cell_str)?
            }
            _ => {
                // 自動取得: 優先順位
                // 1. ref_column (end_ref_column) で指定された列のいずれかが空欄ではない最終行
                // 2. いずれかの列のデータ最終行
                // 3. オートフィルタ対象範囲（右下セル）
                // 4. UsedRange (range.end())
                if !end_ref_cols.is_empty() {
                    detect_end_cell(&range, &end_ref_cols)
                        .or_else(|| detect_end_cell(&range, &[]))
                        .or_else(|| autofilter.map(|(_, end)| end))
                        .unwrap_or_else(|| range.end().unwrap_or((0, 0)))
                } else {
                    detect_end_cell(&range, &[])
                        .or_else(|| autofilter.map(|(_, end)| end))
                        .unwrap_or_else(|| range.end().unwrap_or((0, 0)))
                }
            }
        };

        // 安全なクランプ: 先頭 ≤ 末尾
        let (r1, c1, r2, c2) = (r1.min(r2), c1.min(c2), r1.max(r2), c1.max(c2));

        self.read_range(&range, r1, c1, r2, c2)
    }

    fn field_names(&self) -> Result<Vec<String>> {
        use calamine::Reader;

        // 後方互換: 旧 range/cell フィールド
        if let Some(spec) = self
            .range
            .as_deref()
            .or(self.cell.as_deref())
            .filter(|s| !s.is_empty())
        {
            if self.start_cell.is_none() && self.end_cell.is_none() {
                let ((r1, c1), (_r2, c2)) = parse_a1_range(spec)?;
                let mut wb = calamine::open_workbook_auto(&self.path)?;
                let range = wb.worksheet_range(&self.sheet)?;
                let cell_at = |r: u32, c: u32| {
                    range
                        .get_value((r, c))
                        .map(cell_to_string)
                        .unwrap_or_default()
                };
                if self.has_header {
                    return Ok((c1..=c2)
                        .map(|c| {
                            let h = cell_at(r1, c);
                            if h.is_empty() {
                                col_to_letters(c)
                            } else {
                                h
                            }
                        })
                        .collect());
                } else {
                    return Ok((c1..=c2).map(col_to_letters).collect());
                }
            }
        }

        let mut wb = calamine::open_workbook_auto(&self.path)?;
        let range = wb.worksheet_range(&self.sheet)?;

        let start_ref_str = self
            .start_ref_column
            .as_deref()
            .or(self.ref_column.as_deref());
        let start_ref_cols = match start_ref_str {
            Some(s) if !s.is_empty() => parse_ref_columns(s)?,
            _ => Vec::new(),
        };

        let autofilter = read_autofilter_ref(&self.path, &self.sheet);

        let (r1, c1) = match self.start_mode.as_str() {
            "manual" => {
                let cell_str = self
                    .start_cell
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| anyhow!("先頭セルが手動指定ですが、値が入力されていません"))?;
                parse_a1(cell_str)?
            }
            _ => {
                if let Some(((ar, ac), _)) = autofilter {
                    (ar, ac)
                } else if !start_ref_cols.is_empty() {
                    detect_start_cell(&range, &start_ref_cols)
                        .or_else(|| detect_start_cell(&range, &[]))
                        .unwrap_or_else(|| range.start().unwrap_or((0, 0)))
                } else {
                    detect_start_cell(&range, &[])
                        .unwrap_or_else(|| range.start().unwrap_or((0, 0)))
                }
            }
        };

        let c2 = match self.end_mode.as_str() {
            "manual" => {
                if let Some(cell_str) = self.end_cell.as_deref().filter(|s| !s.is_empty()) {
                    parse_a1(cell_str)
                        .map(|(_, c)| c)
                        .unwrap_or_else(|_| range.end().map(|x| x.1).unwrap_or(c1))
                } else {
                    range.end().map(|x| x.1).unwrap_or(c1)
                }
            }
            _ => range.end().map(|x| x.1).unwrap_or(c1),
        };

        let c2 = c1.max(c2);
        let cell_at = |r: u32, c: u32| {
            range
                .get_value((r, c))
                .map(cell_to_string)
                .unwrap_or_default()
        };

        if self.has_header {
            let mut seen = std::collections::HashSet::new();
            let mut headers = Vec::new();
            for c in c1..=c2 {
                let h = cell_at(r1, c);
                let name = if h.is_empty() { col_to_letters(c) } else { h };
                let mut final_name = name.clone();
                let mut counter = 1;
                while !seen.insert(final_name.clone()) {
                    counter += 1;
                    final_name = format!("{name}_{counter}");
                }
                headers.push(final_name);
            }
            Ok(headers)
        } else {
            Ok((c1..=c2).map(col_to_letters).collect())
        }
    }
}

impl ExcelRangeConnector {
    /// 決定した範囲からレコードを読み取る共通処理。
    fn read_range(
        &self,
        range: &calamine::Range<calamine::Data>,
        r1: u32,
        c1: u32,
        r2: u32,
        c2: u32,
    ) -> Result<Vec<Record>> {
        let cell_at = |r: u32, c: u32| {
            range
                .get_value((r, c))
                .map(cell_to_string)
                .unwrap_or_default()
        };

        // ヘッダー行の決定。
        let (header_row, data_start) = if self.has_header && r2 > r1 {
            let headers: Vec<String> = (c1..=c2)
                .map(|c| {
                    let h = cell_at(r1, c);
                    if h.is_empty() {
                        col_to_letters(c)
                    } else {
                        h
                    }
                })
                .collect();
            (headers, r1 + 1)
        } else {
            let headers: Vec<String> = (c1..=c2).map(col_to_letters).collect();
            (headers, r1)
        };

        let mut out = Vec::new();
        for r in data_start..=r2 {
            let pairs: Vec<(String, String)> = (c1..=c2)
                .enumerate()
                .map(|(i, c)| (header_row[i].clone(), cell_at(r, c)))
                .collect();
            if pairs.iter().all(|(_, v)| v.is_empty()) {
                continue;
            }
            let row_num = (r + 1) as i64;
            out.push(make_record_with_row(
                pairs,
                &self.path,
                &self.common,
                Some(row_num),
            ));
        }
        Ok(out)
    }
}

// ---- Excel: シート名一覧 ----
#[derive(Deserialize)]
struct ExcelSheetsConnector {
    path: String,
}

impl Connector for ExcelSheetsConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        use calamine::Reader;
        let wb = calamine::open_workbook_auto(&self.path)?;
        let cfg = CommonCfg::default();
        let out = wb
            .sheet_names()
            .into_iter()
            .map(|name| make_record(vec![("シート名".to_string(), name)], &self.path, &cfg))
            .collect();
        Ok(out)
    }
}

/// UNIX 秒を "YYYY-MM-DD HH:MM:SS"(UTC)へ。chrono 非依存の最小実装。
pub(crate) fn fmt_epoch(secs: i64) -> String {
    if secs <= 0 {
        return String::new();
    }
    let days = secs.div_euclid(86400);
    let tod = secs.rem_euclid(86400);
    let (h, mi, s) = (tod / 3600, (tod % 3600) / 60, tod % 60);
    // civil_from_days (Howard Hinnant)
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02} {h:02}:{mi:02}:{s:02}")
}

#[cfg(test)]
#[derive(Deserialize)]
struct SourceTypeManifestEntry {
    id: String,
}

#[cfg(test)]
#[test]
fn source_type_manifest_matches_factory_registry() {
    let manifest: Vec<SourceTypeManifestEntry> =
        serde_json::from_str(include_str!("../../shared/source-types.json"))
            .expect("shared source type manifest must be valid JSON");
    let manifest_ids: Vec<&str> = manifest.iter().map(|entry| entry.id.as_str()).collect();

    assert_eq!(manifest_ids, SUPPORTED_SOURCE_TYPES);
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

// ---- Notion データベース（要件 M6 / REST + インテグレーショントークン）----
#[derive(Deserialize)]
struct NotionConnector {
    token: String,
    database_id: String,
    #[serde(flatten)]
    common: CommonCfg,
}

const NOTION_VERSION: &str = "2022-06-28";

/// Notion のプロパティ値 1 個を表示用文字列へ変換する（型ごとに分岐）。
fn notion_prop_to_string(p: &Value) -> String {
    let t = p.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let rich = |arr: Option<&Value>| -> String {
        arr.and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.get("plain_text").and_then(|v| v.as_str()))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default()
    };
    match t {
        "title" | "rich_text" => rich(p.get(t)),
        "number" => match p.get("number") {
            Some(Value::Number(n)) => n.to_string(),
            _ => String::new(),
        },
        "select" | "status" => p
            .get(t)
            .and_then(|s| s.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "multi_select" => p
            .get("multi_select")
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.get("name").and_then(|v| v.as_str()))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default(),
        "date" => {
            let d = p.get("date");
            let start = d
                .and_then(|x| x.get("start"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let end = d.and_then(|x| x.get("end")).and_then(|v| v.as_str());
            match end {
                Some(e) if !e.is_empty() => format!("{start} ~ {e}"),
                _ => start.to_string(),
            }
        }
        "checkbox" => match p.get("checkbox").and_then(|v| v.as_bool()) {
            Some(true) => "✓".to_string(),
            _ => String::new(),
        },
        "url" | "email" | "phone_number" | "created_time" | "last_edited_time" => {
            p.get(t).and_then(|v| v.as_str()).unwrap_or("").to_string()
        }
        "people" => p
            .get("people")
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.get("name").and_then(|v| v.as_str()))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default(),
        "files" => p
            .get("files")
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.get("name").and_then(|v| v.as_str()))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default(),
        "formula" => {
            let f = p.get("formula");
            let ft = f
                .and_then(|x| x.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match ft {
                "string" => f
                    .and_then(|x| x.get("string"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                "number" => f
                    .and_then(|x| x.get("number"))
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                "boolean" => match f.and_then(|x| x.get("boolean")).and_then(|v| v.as_bool()) {
                    Some(true) => "✓".to_string(),
                    _ => String::new(),
                },
                "date" => f
                    .and_then(|x| x.get("date"))
                    .and_then(|x| x.get("start"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                _ => String::new(),
            }
        }
        "relation" => {
            let n = p
                .get("relation")
                .and_then(|a| a.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            if n > 0 {
                format!("{n} 件")
            } else {
                String::new()
            }
        }
        "unique_id" => {
            let num = p.get("unique_id").and_then(|x| x.get("number"));
            let prefix = p
                .get("unique_id")
                .and_then(|x| x.get("prefix"))
                .and_then(|v| v.as_str());
            match (prefix, num) {
                (Some(pre), Some(n)) => format!("{pre}-{n}"),
                (None, Some(n)) => n.to_string(),
                _ => String::new(),
            }
        }
        _ => String::new(),
    }
}

/// Notion のページ 1 件（properties + url）を共通 Record へ正規化する。
fn notion_page_to_record(page: &Value, common: &CommonCfg) -> Record {
    let page_url = page.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let props = page.get("properties").and_then(|v| v.as_object());
    let mut pairs: Vec<(String, String)> = Vec::new();
    if let Some(props) = props {
        for (name, p) in props {
            pairs.push((name.clone(), notion_prop_to_string(p)));
        }
    }
    let mut rec = make_record(pairs, page_url, common);
    // label 未指定なら title 型プロパティを主見出しにする（Notion 特有）。
    if common.label_column.is_none() {
        if let Some(props) = props {
            for (_name, p) in props {
                if p.get("type").and_then(|v| v.as_str()) == Some("title") {
                    rec.label = Some(notion_prop_to_string(p));
                    break;
                }
            }
        }
    }
    rec
}

impl Connector for NotionConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let url = format!(
            "https://api.notion.com/v1/databases/{}/query",
            self.database_id
        );
        let mut out = Vec::new();
        let mut start_cursor: Option<String> = None;

        loop {
            let mut body = serde_json::json!({ "page_size": 100 });
            if let Some(c) = &start_cursor {
                body["start_cursor"] = Value::String(c.clone());
            }
            let resp = ureq::post(&url)
                .set("Authorization", &format!("Bearer {}", self.token))
                .set("Notion-Version", NOTION_VERSION)
                .set("Content-Type", "application/json")
                .send_json(body);

            let value: Value = match resp {
                Ok(r) => r.into_json()?,
                Err(ureq::Error::Status(code, r)) => {
                    let msg = r.into_string().unwrap_or_default();
                    return Err(anyhow!("Notion API エラー {code}: {msg}"));
                }
                Err(e) => return Err(anyhow!("Notion への接続に失敗: {e}")),
            };

            if let Some(results) = value.get("results").and_then(|v| v.as_array()) {
                for page in results {
                    out.push(notion_page_to_record(page, &self.common));
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
}

// ---- Microsoft Graph 系（要件 M5 / EntraID ユーザー・グループ・Teams）----
#[derive(Deserialize)]
struct GraphAuth {
    tenant_id: String,
    client_id: String,
    client_secret: String,
}

/// Graph の JSON スカラー値（$select したフィールド）を表示用文字列へ。
pub(crate) fn json_scalar_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(a) => a
            .iter()
            .map(json_scalar_to_string)
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    }
}

/// Graph のフラットなオブジェクト（user/group 等）を共通 Record へ。
fn graph_flat_record(
    obj: &Value,
    fields: &[&str],
    label_field: &str,
    open_ref: Option<String>,
    common: &CommonCfg,
) -> Record {
    let pairs: Vec<(String, String)> = fields
        .iter()
        .map(|f| {
            (
                (*f).to_string(),
                obj.get(*f).map(json_scalar_to_string).unwrap_or_default(),
            )
        })
        .collect();
    let mut rec = make_record(pairs, "", common);
    if common.label_column.is_none() {
        rec.label = obj
            .get(label_field)
            .and_then(|v| v.as_str())
            .map(String::from);
    }
    if common.open_ref_column.is_none() {
        rec.open_ref = open_ref;
    }
    rec
}

fn mailto_open_ref(obj: &Value) -> Option<String> {
    obj.get("mail")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|m| format!("mailto:{m}"))
}

// EntraID ユーザー
#[derive(Deserialize)]
struct GraphUsersConnector {
    #[serde(flatten)]
    auth: GraphAuth,
    #[serde(flatten)]
    common: CommonCfg,
}
impl Connector for GraphUsersConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let client = GraphClient::new(
            &self.auth.tenant_id,
            &self.auth.client_id,
            &self.auth.client_secret,
        )?;
        let url = "https://graph.microsoft.com/v1.0/users?$select=displayName,userPrincipalName,mail,jobTitle,department,officeLocation,mobilePhone,id&$top=999";
        let fields = [
            "displayName",
            "userPrincipalName",
            "mail",
            "jobTitle",
            "department",
            "officeLocation",
            "mobilePhone",
        ];
        Ok(client
            .get_all(url)?
            .iter()
            .map(|u| graph_flat_record(u, &fields, "displayName", mailto_open_ref(u), &self.common))
            .collect())
    }
}

// EntraID グループ
#[derive(Deserialize)]
struct GraphGroupsConnector {
    #[serde(flatten)]
    auth: GraphAuth,
    #[serde(flatten)]
    common: CommonCfg,
}
impl Connector for GraphGroupsConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let client = GraphClient::new(
            &self.auth.tenant_id,
            &self.auth.client_id,
            &self.auth.client_secret,
        )?;
        let url = "https://graph.microsoft.com/v1.0/groups?$select=displayName,mail,description,visibility,id&$top=999";
        let fields = ["displayName", "mail", "description", "visibility"];
        Ok(client
            .get_all(url)?
            .iter()
            .map(|g| graph_flat_record(g, &fields, "displayName", mailto_open_ref(g), &self.common))
            .collect())
    }
}

// Teams チームリスト（Team 化されたグループ）
#[derive(Deserialize)]
struct GraphTeamsConnector {
    #[serde(flatten)]
    auth: GraphAuth,
    #[serde(flatten)]
    common: CommonCfg,
}
impl Connector for GraphTeamsConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let client = GraphClient::new(
            &self.auth.tenant_id,
            &self.auth.client_id,
            &self.auth.client_secret,
        )?;
        let url = "https://graph.microsoft.com/v1.0/groups?$filter=resourceProvisioningOptions/Any(x:x eq 'Team')&$select=displayName,mail,description,id&$top=999";
        let fields = ["displayName", "mail", "description"];
        Ok(client
            .get_all(url)?
            .iter()
            .map(|t| graph_flat_record(t, &fields, "displayName", None, &self.common))
            .collect())
    }
}

// ---- SharePoint 共通（サイト ID 解決は crate::graph::site_id に集約）----
use crate::graph::site_id as graph_site_id;

/// SharePoint リスト項目の fields から、システム列を除いた表示対象かを判定。
fn sp_field_is_display(key: &str) -> bool {
    if key.starts_with('_') || key.starts_with('@') {
        return false;
    }
    const BLOCK: &[&str] = &[
        "ContentType",
        "Attachments",
        "Edit",
        "LinkTitleNoMenu",
        "LinkTitle",
        "LinkTitle2",
        "ItemChildCount",
        "FolderChildCount",
        "AuthorLookupId",
        "EditorLookupId",
        "AppAuthorLookupId",
        "AppEditorLookupId",
    ];
    !BLOCK.contains(&key)
}

// ---- SharePoint リスト ----
#[derive(Deserialize)]
struct GraphSharePointListConnector {
    #[serde(flatten)]
    auth: GraphAuth,
    site_url: String,
    list_name: String,
    #[serde(flatten)]
    common: CommonCfg,
}
impl Connector for GraphSharePointListConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let client = GraphClient::new(
            &self.auth.tenant_id,
            &self.auth.client_id,
            &self.auth.client_secret,
        )?;
        let sid = graph_site_id(&client, &self.site_url)?;
        // リスト名（内部名 or 表示名）から id を解決。
        let lists = client.get(&format!(
            "https://graph.microsoft.com/v1.0/sites/{sid}/lists?$select=id,name,displayName"
        ))?;
        let lid = lists
            .get("value")
            .and_then(|v| v.as_array())
            .and_then(|arr| {
                arr.iter().find(|l| {
                    let n = l.get("name").and_then(|v| v.as_str());
                    let d = l.get("displayName").and_then(|v| v.as_str());
                    n == Some(self.list_name.as_str()) || d == Some(self.list_name.as_str())
                })
            })
            .and_then(|l| l.get("id").and_then(|v| v.as_str()))
            .ok_or_else(|| anyhow!("リストが見つかりません: {}", self.list_name))?
            .to_string();

        let url = format!(
            "https://graph.microsoft.com/v1.0/sites/{sid}/lists/{lid}/items?expand=fields&$top=200"
        );
        let mut out = Vec::new();
        for it in client.get_all(&url)? {
            let web = it.get("webUrl").and_then(|v| v.as_str()).map(String::from);
            let mut pairs = Vec::new();
            let fields = it.get("fields").and_then(|v| v.as_object());
            if let Some(f) = fields {
                for (k, v) in f {
                    if sp_field_is_display(k) {
                        pairs.push((k.clone(), json_scalar_to_string(v)));
                    }
                }
            }
            let mut rec = make_record(pairs, web.as_deref().unwrap_or(""), &self.common);
            if self.common.open_ref_column.is_none() {
                rec.open_ref = web;
            }
            if self.common.label_column.is_none() {
                if let Some(title) = fields.and_then(|f| f.get("Title")).and_then(|v| v.as_str()) {
                    rec.label = Some(title.to_string());
                }
            }
            out.push(rec);
        }
        Ok(out)
    }
}

// ---- SharePoint ドキュメントライブラリ ----
#[derive(Deserialize)]
struct GraphSharePointLibraryConnector {
    #[serde(flatten)]
    auth: GraphAuth,
    site_url: String,
    #[serde(default)]
    library_name: Option<String>,
    #[serde(flatten)]
    common: CommonCfg,
}
impl Connector for GraphSharePointLibraryConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let client = GraphClient::new(
            &self.auth.tenant_id,
            &self.auth.client_id,
            &self.auth.client_secret,
        )?;
        let sid = graph_site_id(&client, &self.site_url)?;
        // ライブラリ名指定があれば drives から一致検索、無ければ既定ライブラリ /drive。
        let did = match self.library_name.as_deref().filter(|s| !s.is_empty()) {
            Some(name) => {
                let drives = client.get(&format!(
                    "https://graph.microsoft.com/v1.0/sites/{sid}/drives?$select=id,name"
                ))?;
                drives
                    .get("value")
                    .and_then(|v| v.as_array())
                    .and_then(|arr| {
                        arr.iter()
                            .find(|d| d.get("name").and_then(|v| v.as_str()) == Some(name))
                    })
                    .and_then(|d| d.get("id").and_then(|v| v.as_str()))
                    .ok_or_else(|| anyhow!("ライブラリが見つかりません: {name}"))?
                    .to_string()
            }
            None => client
                .get(&format!(
                    "https://graph.microsoft.com/v1.0/sites/{sid}/drive?$select=id"
                ))?
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("既定ライブラリを取得できません"))?
                .to_string(),
        };

        let mut out = Vec::new();
        walk_drive(
            &client,
            &format!("sites/{sid}/drives/{did}"),
            "root",
            &self.common,
            &mut out,
        )?;
        Ok(out)
    }
}

/// ドライブ配下を再帰的に走査してファイル/フォルダをレコード化する。
/// `drive_base` は "sites/{sid}/drives/{did}" または "users/{upn}/drive" のような
/// Graph の drive リソースパス（末尾スラッシュ無し）。
fn walk_drive(
    client: &GraphClient,
    drive_base: &str,
    item_id: &str,
    common: &CommonCfg,
    out: &mut Vec<Record>,
) -> Result<()> {
    let url =
        format!("https://graph.microsoft.com/v1.0/{drive_base}/items/{item_id}/children?$top=200");
    for c in client.get_all(&url)? {
        let is_folder = c.get("folder").is_some();
        let name = c
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let web = c.get("webUrl").and_then(|v| v.as_str()).map(String::from);
        let size = c.get("size").map(json_scalar_to_string).unwrap_or_default();
        let modified = c
            .get("lastModifiedDateTime")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let path = c
            .get("parentReference")
            .and_then(|p| p.get("path"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let pairs = vec![
            ("名前".to_string(), name.clone()),
            (
                "種類".to_string(),
                if is_folder {
                    "フォルダ"
                } else {
                    "ファイル"
                }
                .to_string(),
            ),
            ("サイズ".to_string(), size),
            ("更新日時".to_string(), modified),
            ("パス".to_string(), path),
        ];
        let mut rec = make_record(pairs, web.as_deref().unwrap_or(""), common);
        if common.open_ref_column.is_none() {
            rec.open_ref = web;
        }
        if common.label_column.is_none() {
            rec.label = Some(name);
        }
        out.push(rec);

        if is_folder {
            if let Some(id) = c.get("id").and_then(|v| v.as_str()) {
                walk_drive(client, drive_base, id, common, out)?;
            }
        }
    }
    Ok(())
}

// ---- Entra ID デバイス一覧（要件: M365 & デバイス/ライセンス管理系）----
#[derive(Deserialize)]
struct GraphDevicesConnector {
    #[serde(flatten)]
    auth: GraphAuth,
    #[serde(flatten)]
    common: CommonCfg,
}
impl Connector for GraphDevicesConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let client = GraphClient::new(
            &self.auth.tenant_id,
            &self.auth.client_id,
            &self.auth.client_secret,
        )?;
        let url = "https://graph.microsoft.com/v1.0/devices?$select=displayName,operatingSystem,operatingSystemVersion,isCompliant,trustType,approximateLastSignInDateTime&$top=999";
        let fields = [
            "displayName",
            "operatingSystem",
            "operatingSystemVersion",
            "isCompliant",
            "trustType",
            "approximateLastSignInDateTime",
        ];
        Ok(client
            .get_all(url)?
            .iter()
            .map(|d| graph_flat_record(d, &fields, "displayName", None, &self.common))
            .collect())
    }
}

// ---- Intune デバイス一覧 ----
#[derive(Deserialize)]
struct GraphIntuneDevicesConnector {
    #[serde(flatten)]
    auth: GraphAuth,
    #[serde(flatten)]
    common: CommonCfg,
}
impl Connector for GraphIntuneDevicesConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let client = GraphClient::new(
            &self.auth.tenant_id,
            &self.auth.client_id,
            &self.auth.client_secret,
        )?;
        let url = "https://graph.microsoft.com/v1.0/deviceManagement/managedDevices?$select=deviceName,operatingSystem,osVersion,complianceState,userPrincipalName,lastSyncDateTime&$top=999";
        let fields = [
            "deviceName",
            "operatingSystem",
            "osVersion",
            "complianceState",
            "userPrincipalName",
            "lastSyncDateTime",
        ];
        Ok(client
            .get_all(url)?
            .iter()
            .map(|d| graph_flat_record(d, &fields, "deviceName", None, &self.common))
            .collect())
    }
}

// ---- Microsoft 365 ライセンス一覧 ----
#[derive(Deserialize)]
struct GraphLicensesConnector {
    #[serde(flatten)]
    auth: GraphAuth,
    #[serde(flatten)]
    common: CommonCfg,
}
impl Connector for GraphLicensesConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let client = GraphClient::new(
            &self.auth.tenant_id,
            &self.auth.client_id,
            &self.auth.client_secret,
        )?;
        let url = "https://graph.microsoft.com/v1.0/subscribedSkus?$select=skuPartNumber,skuId,consumedUnits,prepaidUnits,capabilityStatus";
        let mut out = Vec::new();
        for sku in client.get_all(url)? {
            let part_number = sku
                .get("skuPartNumber")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let consumed = sku
                .get("consumedUnits")
                .map(json_scalar_to_string)
                .unwrap_or_default();
            let enabled = sku
                .get("prepaidUnits")
                .and_then(|p| p.get("enabled"))
                .map(json_scalar_to_string)
                .unwrap_or_default();
            let remaining = match (enabled.parse::<i64>(), consumed.parse::<i64>()) {
                (Ok(e), Ok(c)) => (e - c).to_string(),
                _ => String::new(),
            };
            let pairs = vec![
                ("ライセンス名".to_string(), part_number.clone()),
                (
                    "SKU ID".to_string(),
                    sku.get("skuId")
                        .map(json_scalar_to_string)
                        .unwrap_or_default(),
                ),
                ("総数".to_string(), enabled),
                ("消費数".to_string(), consumed),
                ("残数".to_string(), remaining),
                (
                    "状態".to_string(),
                    sku.get("capabilityStatus")
                        .map(json_scalar_to_string)
                        .unwrap_or_default(),
                ),
            ];
            let mut rec = make_record(pairs, "", &self.common);
            if self.common.label_column.is_none() {
                rec.label = Some(part_number);
            }
            out.push(rec);
        }
        Ok(out)
    }
}

// ---- Outlook 予定表（要件: 対象ユーザーの UPN を指定）----
#[derive(Deserialize)]
struct GraphCalendarConnector {
    #[serde(flatten)]
    auth: GraphAuth,
    /// 対象ユーザーの UPN（メールアドレス）。
    user: String,
    #[serde(flatten)]
    common: CommonCfg,
}
impl Connector for GraphCalendarConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let client = GraphClient::new(
            &self.auth.tenant_id,
            &self.auth.client_id,
            &self.auth.client_secret,
        )?;
        let user = urlencoding_light(&self.user);
        let url = format!(
            "https://graph.microsoft.com/v1.0/users/{user}/events?$select=subject,start,end,location,organizer,attendees,webLink&$top=250&$orderby=start/dateTime desc"
        );
        let mut out = Vec::new();
        for ev in client.get_all(&url)? {
            let subject = ev
                .get("subject")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let start = ev
                .get("start")
                .and_then(|s| s.get("dateTime"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let end = ev
                .get("end")
                .and_then(|s| s.get("dateTime"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let location = ev
                .get("location")
                .and_then(|l| l.get("displayName"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let organizer = ev
                .get("organizer")
                .and_then(|o| o.get("emailAddress"))
                .and_then(|e| e.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let attendees = ev
                .get("attendees")
                .and_then(|a| a.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|a| a.get("emailAddress")?.get("name")?.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            let web_link = ev.get("webLink").and_then(|v| v.as_str()).map(String::from);
            let pairs = vec![
                ("件名".to_string(), subject.clone()),
                ("開始".to_string(), start),
                ("終了".to_string(), end),
                ("場所".to_string(), location),
                ("主催者".to_string(), organizer),
                ("参加者".to_string(), attendees),
            ];
            let mut rec = make_record(pairs, "", &self.common);
            if self.common.label_column.is_none() {
                rec.label = Some(subject);
            }
            if self.common.open_ref_column.is_none() {
                rec.open_ref = web_link;
            }
            out.push(rec);
        }
        Ok(out)
    }
}

// ---- OneDrive（対象ユーザーの UPN を指定。既存 walk_drive を再利用）----
#[derive(Deserialize)]
struct GraphOneDriveConnector {
    #[serde(flatten)]
    auth: GraphAuth,
    /// 対象ユーザーの UPN（メールアドレス）。
    user: String,
    #[serde(flatten)]
    common: CommonCfg,
}
impl Connector for GraphOneDriveConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let client = GraphClient::new(
            &self.auth.tenant_id,
            &self.auth.client_id,
            &self.auth.client_secret,
        )?;
        let user = urlencoding_light(&self.user);
        let mut out = Vec::new();
        walk_drive(
            &client,
            &format!("users/{user}/drive"),
            "root",
            &self.common,
            &mut out,
        )?;
        Ok(out)
    }
}

// ---- Teams タスク & Planner（グループ ID 単位で配下の全プランのタスクを集約）----
#[derive(Deserialize)]
struct GraphPlannerConnector {
    #[serde(flatten)]
    auth: GraphAuth,
    /// タスクを読みたい Microsoft 365 グループ（チーム）の ID。
    group_id: String,
    #[serde(flatten)]
    common: CommonCfg,
}
impl Connector for GraphPlannerConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let client = GraphClient::new(
            &self.auth.tenant_id,
            &self.auth.client_id,
            &self.auth.client_secret,
        )?;
        let plans = client.get(&format!(
            "https://graph.microsoft.com/v1.0/groups/{}/planner/plans?$select=id,title",
            self.group_id
        ))?;
        let mut out = Vec::new();
        let Some(plans) = plans.get("value").and_then(|v| v.as_array()) else {
            return Ok(out);
        };
        for plan in plans {
            let plan_id = plan
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let plan_title = plan
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if plan_id.is_empty() {
                continue;
            }
            // bucket(列) の ID → 名前 の対応表を先に作る。
            let buckets = client.get(&format!(
                "https://graph.microsoft.com/v1.0/planner/plans/{plan_id}/buckets?$select=id,name"
            ))?;
            let bucket_names: std::collections::HashMap<String, String> = buckets
                .get("value")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|b| {
                            Some((
                                b.get("id")?.as_str()?.to_string(),
                                b.get("name")?.as_str()?.to_string(),
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default();

            let tasks_url = format!(
                "https://graph.microsoft.com/v1.0/planner/plans/{plan_id}/tasks?$select=title,dueDateTime,percentComplete,bucketId,assignments"
            );
            for t in client.get_all(&tasks_url)? {
                let title = t
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let due = t
                    .get("dueDateTime")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let percent = t
                    .get("percentComplete")
                    .map(json_scalar_to_string)
                    .unwrap_or_default();
                let status = match percent.as_str() {
                    "100" => "完了",
                    "0" => "未着手",
                    "" => "",
                    _ => "進行中",
                };
                let bucket = t
                    .get("bucketId")
                    .and_then(|v| v.as_str())
                    .and_then(|id| bucket_names.get(id))
                    .cloned()
                    .unwrap_or_default();
                let assignee_count = t
                    .get("assignments")
                    .and_then(|a| a.as_object())
                    .map(|o| o.len())
                    .unwrap_or(0);
                let pairs = vec![
                    ("プラン".to_string(), plan_title.clone()),
                    ("タスク名".to_string(), title.clone()),
                    ("Bucket".to_string(), bucket),
                    ("期限".to_string(), due),
                    ("進行状況".to_string(), status.to_string()),
                    ("担当者数".to_string(), assignee_count.to_string()),
                ];
                let mut rec = make_record(pairs, "", &self.common);
                if self.common.label_column.is_none() {
                    rec.label = Some(title);
                }
                out.push(rec);
            }
        }
        Ok(out)
    }
}

/// UPN 等をパス埋め込み用に最小限エスケープ（英数字・@・.・-・_ 以外を %XX に）。
pub(crate) fn urlencoding_light(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'@' | b'.' | b'-' | b'_' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod graph_live_tests {
    use super::*;

    fn env(k: &str) -> Option<String> {
        std::env::var(k).ok().filter(|s| !s.is_empty())
    }

    // ライブ結合テスト。環境変数を設定して実行:
    //   $env:EVERYDB_GRAPH_TENANT=..; $env:EVERYDB_GRAPH_CLIENT=..; $env:EVERYDB_GRAPH_SECRET=..;
    //   $env:EVERYDB_SP_SITE='https://.../sites/..'; cargo test -- --ignored graph_live
    #[test]
    #[ignore = "requires EVERYDB_GRAPH_* env vars + admin-consented app"]
    fn graph_live_all() {
        let tenant = env("EVERYDB_GRAPH_TENANT").expect("EVERYDB_GRAPH_TENANT");
        let client = env("EVERYDB_GRAPH_CLIENT").expect("EVERYDB_GRAPH_CLIENT");
        let secret = env("EVERYDB_GRAPH_SECRET").expect("EVERYDB_GRAPH_SECRET");
        let auth = serde_json::json!({ "tenant_id": tenant, "client_id": client, "client_secret": secret });

        let run = |type_: &str, extra: serde_json::Value| -> Vec<Record> {
            let mut cfg = auth.clone();
            for (k, v) in extra.as_object().unwrap() {
                cfg[k] = v.clone();
            }
            build_connector(type_, &cfg.to_string())
                .unwrap()
                .fetch()
                .unwrap()
        };

        let users = run("graph_entra_users", serde_json::json!({}));
        println!("users = {}", users.len());
        assert!(!users.is_empty());

        let groups = run("graph_entra_groups", serde_json::json!({}));
        println!("groups = {}", groups.len());
        assert!(!groups.is_empty());

        let teams = run("graph_teams", serde_json::json!({}));
        println!("teams = {}", teams.len());
        assert!(!teams.is_empty());

        if let Some(site) = env("EVERYDB_SP_SITE") {
            let list = run(
                "graph_sharepoint_list",
                serde_json::json!({ "site_url": site, "list_name": "SampleList" }),
            );
            println!("sharepoint list = {}", list.len());
            assert!(!list.is_empty());
            assert!(list.iter().all(|r| r
                .open_ref
                .as_deref()
                .map(|u| u.starts_with("http"))
                .unwrap_or(false)));

            let lib = run(
                "graph_sharepoint_library",
                serde_json::json!({ "site_url": site }),
            );
            println!("sharepoint library = {}", lib.len());
            assert!(!lib.is_empty());
        }
    }
}

// ---- ブラウザのブックマーク & 履歴（要件 REQ-新規: ローカルブラウザデータ系）----
// 実行中のブラウザは History/places.sqlite をロックすることがあるため、
// 読み取り前に一時ファイルへコピーしてからそのコピーを開く。
fn copy_locked_sqlite(path: &str) -> Result<std::path::PathBuf> {
    let src = std::path::Path::new(path);
    let file_name = src
        .file_name()
        .ok_or_else(|| anyhow!("不正なパス: {path}"))?;
    let tmp = std::env::temp_dir().join(format!(
        "everydb_{}_{}",
        std::process::id(),
        file_name.to_string_lossy()
    ));
    std::fs::copy(src, &tmp).map_err(|e| {
        anyhow!("ファイルのコピーに失敗（ブラウザ実行中はロックされる場合があります）: {e}")
    })?;
    Ok(tmp)
}

/// Chrome/Edge/Brave（WebKit 拡張エポック: 1601-01-01 からの μ秒）→ Unix 秒。
fn webkit_micros_to_unix_secs(us: i64) -> i64 {
    us / 1_000_000 - 11_644_473_600
}

const HISTORY_ROW_CAP: usize = 10_000;

#[derive(Deserialize)]
struct BrowserHistoryConnector {
    /// "chrome" | "edge" | "brave" | "vivaldi" | "firefox"
    browser: String,
    path: String,
    #[serde(flatten)]
    common: CommonCfg,
}

impl Connector for BrowserHistoryConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let tmp = copy_locked_sqlite(&self.path)?;
        let result = (|| -> Result<Vec<Record>> {
            let conn = Connection::open_with_flags(&tmp, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            if self.browser == "firefox" {
                self.fetch_firefox(&conn)
            } else {
                self.fetch_chromium(&conn)
            }
        })();
        let _ = std::fs::remove_file(&tmp);
        result
    }
}

impl BrowserHistoryConnector {
    fn fetch_chromium(&self, conn: &Connection) -> Result<Vec<Record>> {
        let mut stmt = conn.prepare(
            "SELECT url, title, visit_count, last_visit_time FROM urls
             ORDER BY last_visit_time DESC LIMIT ?1",
        )?;
        let mut out = Vec::new();
        let mut rows = stmt.query([HISTORY_ROW_CAP as i64])?;
        while let Some(r) = rows.next()? {
            let url: String = r.get(0)?;
            let title: String = r.get::<_, Option<String>>(1)?.unwrap_or_default();
            let visits: i64 = r.get(2)?;
            let last_visit_us: i64 = r.get(3)?;
            let last_visit = fmt_epoch(webkit_micros_to_unix_secs(last_visit_us));
            let pairs = vec![
                ("タイトル".to_string(), title),
                ("URL".to_string(), url.clone()),
                ("訪問回数".to_string(), visits.to_string()),
                ("最終訪問".to_string(), last_visit),
            ];
            out.push(make_record(pairs, &url, &self.common));
        }
        Ok(out)
    }

    fn fetch_firefox(&self, conn: &Connection) -> Result<Vec<Record>> {
        let mut stmt = conn.prepare(
            "SELECT url, title, visit_count, last_visit_date FROM moz_places
             WHERE last_visit_date IS NOT NULL
             ORDER BY last_visit_date DESC LIMIT ?1",
        )?;
        let mut out = Vec::new();
        let mut rows = stmt.query([HISTORY_ROW_CAP as i64])?;
        while let Some(r) = rows.next()? {
            let url: String = r.get(0)?;
            let title: String = r.get::<_, Option<String>>(1)?.unwrap_or_default();
            let visits: i64 = r.get::<_, Option<i64>>(2)?.unwrap_or(0);
            let last_visit_us: i64 = r.get(3)?;
            let last_visit = fmt_epoch(last_visit_us / 1_000_000);
            let pairs = vec![
                ("タイトル".to_string(), title),
                ("URL".to_string(), url.clone()),
                ("訪問回数".to_string(), visits.to_string()),
                ("最終訪問".to_string(), last_visit),
            ];
            out.push(make_record(pairs, &url, &self.common));
        }
        Ok(out)
    }
}

#[derive(Deserialize)]
struct BrowserBookmarksConnector {
    /// "chrome" | "edge" | "brave" | "vivaldi" | "firefox"
    browser: String,
    path: String,
    #[serde(flatten)]
    common: CommonCfg,
}

impl Connector for BrowserBookmarksConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        if self.browser == "firefox" {
            self.fetch_firefox()
        } else {
            self.fetch_chromium()
        }
    }
}

impl BrowserBookmarksConnector {
    /// Chromium 系の Bookmarks は JSON ファイル（ブラウザ実行中でもロックされない）。
    fn fetch_chromium(&self) -> Result<Vec<Record>> {
        let text = std::fs::read_to_string(&self.path)?;
        let json: Value = serde_json::from_str(&text)?;
        let mut out = Vec::new();
        if let Some(roots) = json.get("roots").and_then(|v| v.as_object()) {
            for (_root_name, root) in roots {
                self.walk_chromium_node(root, "", &mut out);
            }
        }
        Ok(out)
    }

    fn walk_chromium_node(&self, node: &Value, folder_path: &str, out: &mut Vec<Record>) {
        let node_type = node.get("type").and_then(|v| v.as_str()).unwrap_or("");
        match node_type {
            "url" => {
                let name = node
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let url = node
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let date_added = node
                    .get("date_added")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<i64>().ok())
                    .map(|us| fmt_epoch(webkit_micros_to_unix_secs(us)))
                    .unwrap_or_default();
                let pairs = vec![
                    ("名前".to_string(), name),
                    ("URL".to_string(), url.clone()),
                    ("フォルダ".to_string(), folder_path.to_string()),
                    ("追加日時".to_string(), date_added),
                ];
                out.push(make_record(pairs, &url, &self.common));
            }
            "folder" => {
                let name = node.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let new_path = if folder_path.is_empty() {
                    name.to_string()
                } else {
                    format!("{folder_path}/{name}")
                };
                if let Some(children) = node.get("children").and_then(|v| v.as_array()) {
                    for child in children {
                        self.walk_chromium_node(child, &new_path, out);
                    }
                }
            }
            _ => {}
        }
    }

    /// Firefox のブックマークは places.sqlite 内（moz_bookmarks + moz_places）。
    /// フォルダ階層の再構築は行わず、名前/URL/追加日時のみを扱う（簡易版）。
    fn fetch_firefox(&self) -> Result<Vec<Record>> {
        let tmp = copy_locked_sqlite(&self.path)?;
        let result = (|| -> Result<Vec<Record>> {
            let conn = Connection::open_with_flags(&tmp, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            let mut stmt = conn.prepare(
                "SELECT b.title, p.url, b.dateAdded FROM moz_bookmarks b
                 JOIN moz_places p ON b.fk = p.id
                 WHERE b.type = 1 ORDER BY b.dateAdded DESC",
            )?;
            let mut out = Vec::new();
            let mut rows = stmt.query([])?;
            while let Some(r) = rows.next()? {
                let title: String = r.get::<_, Option<String>>(0)?.unwrap_or_default();
                let url: String = r.get(1)?;
                let added_us: i64 = r.get::<_, Option<i64>>(2)?.unwrap_or(0);
                let pairs = vec![
                    ("名前".to_string(), title),
                    ("URL".to_string(), url.clone()),
                    ("追加日時".to_string(), fmt_epoch(added_us / 1_000_000)),
                ];
                out.push(make_record(pairs, &url, &self.common));
            }
            Ok(out)
        })();
        let _ = std::fs::remove_file(&tmp);
        result
    }
}

/// パス文字列（Windows/Unix いずれの区切りも）からファイル名部分だけを取り出す。
fn basename_of(path: &str) -> String {
    path.rsplit(['\\', '/']).next().unwrap_or(path).to_string()
}

/// `file://` URI の簡易パーセントデコード（英数字と一般的な記号のみ対象の最小実装）。
/// 対象は Firefox のダウンロード履歴の `destinationFileURI` で、スペース等が %XX 形式になる。
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(hex);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `file:///C:/Users/...` のような URI をローカルパスへ変換する（Windows 向けの簡易実装）。
fn file_uri_to_path(uri: &str) -> String {
    let decoded = percent_decode(uri);
    let stripped = decoded
        .strip_prefix("file:///")
        .or_else(|| decoded.strip_prefix("file://"))
        .unwrap_or(&decoded);
    stripped.replace('/', "\\")
}

#[derive(Deserialize)]
struct BrowserDownloadsConnector {
    /// "chrome" | "edge" | "brave" | "vivaldi" | "firefox"（要件: Vivaldi 対応）
    browser: String,
    path: String,
    #[serde(flatten)]
    common: CommonCfg,
}

impl Connector for BrowserDownloadsConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let tmp = copy_locked_sqlite(&self.path)?;
        let result = (|| -> Result<Vec<Record>> {
            let conn = Connection::open_with_flags(&tmp, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            if self.browser == "firefox" {
                self.fetch_firefox(&conn)
            } else {
                self.fetch_chromium(&conn)
            }
        })();
        let _ = std::fs::remove_file(&tmp);
        result
    }
}

impl BrowserDownloadsConnector {
    /// Chromium 系（Chrome/Edge/Brave/Vivaldi）は履歴と同じ `History` ファイル内の
    /// `downloads` + `downloads_url_chains` テーブルを使う（別ファイルではない）。
    /// `state` の意味は Chromium の `download_item.h`（IN_PROGRESS=0, COMPLETE=1,
    /// CANCELLED=2, INTERRUPTED=4）に基づく best-effort のラベル付けで、バージョンにより
    /// ずれる可能性があるため未知の値は番号のまま表示する。
    fn fetch_chromium(&self, conn: &Connection) -> Result<Vec<Record>> {
        let mut stmt = conn.prepare(
            "SELECT d.id, d.target_path, d.start_time, d.end_time, d.received_bytes, d.total_bytes, d.state,
                    (SELECT url FROM downloads_url_chains WHERE id = d.id ORDER BY chain_index DESC LIMIT 1) AS url
             FROM downloads d ORDER BY d.start_time DESC LIMIT ?1",
        )?;
        let mut out = Vec::new();
        let mut rows = stmt.query([HISTORY_ROW_CAP as i64])?;
        while let Some(r) = rows.next()? {
            let target_path: String = r.get(1)?;
            let start_us: i64 = r.get(2)?;
            let end_us: i64 = r.get(3)?;
            let received: i64 = r.get(4)?;
            let total: i64 = r.get(5)?;
            let state: i64 = r.get(6)?;
            let url: String = r.get::<_, Option<String>>(7)?.unwrap_or_default();
            let state_label = match state {
                0 => "ダウンロード中".to_string(),
                1 => "完了".to_string(),
                2 => "キャンセル".to_string(),
                4 => "中断".to_string(),
                other => format!("不明({other})"),
            };
            let size = if total > 0 { total } else { received };
            let pairs = vec![
                ("ファイル名".to_string(), basename_of(&target_path)),
                ("保存先パス".to_string(), target_path.clone()),
                ("URL".to_string(), url.clone()),
                ("状態".to_string(), state_label),
                ("サイズ(バイト)".to_string(), size.to_string()),
                (
                    "開始日時".to_string(),
                    fmt_epoch(webkit_micros_to_unix_secs(start_us)),
                ),
                (
                    "完了日時".to_string(),
                    if end_us > 0 {
                        fmt_epoch(webkit_micros_to_unix_secs(end_us))
                    } else {
                        String::new()
                    },
                ),
            ];
            out.push(make_record(pairs, &target_path, &self.common));
        }
        Ok(out)
    }

    /// Firefox は `places.sqlite` の `moz_annos`(`downloads/destinationFileURI`) を
    /// `moz_places` と結合して復元する（ブックマークと同じ「簡易版」の考え方）。
    fn fetch_firefox(&self, conn: &Connection) -> Result<Vec<Record>> {
        let mut stmt = conn.prepare(
            "SELECT p.url, p.title, a.content, a.dateAdded
             FROM moz_annos a
             JOIN moz_anno_attributes attr ON a.anno_attribute_id = attr.id
             JOIN moz_places p ON a.place_id = p.id
             WHERE attr.name = 'downloads/destinationFileURI'
             ORDER BY a.dateAdded DESC",
        )?;
        let mut out = Vec::new();
        let mut rows = stmt.query([])?;
        while let Some(r) = rows.next()? {
            let url: String = r.get(0)?;
            let title: String = r.get::<_, Option<String>>(1)?.unwrap_or_default();
            let dest_uri: String = r.get(2)?;
            let added_us: i64 = r.get::<_, Option<i64>>(3)?.unwrap_or(0);
            let dest_path = file_uri_to_path(&dest_uri);
            let pairs = vec![
                ("ファイル名".to_string(), basename_of(&dest_path)),
                ("保存先パス".to_string(), dest_path.clone()),
                ("URL".to_string(), url.clone()),
                ("タイトル".to_string(), title),
                ("追加日時".to_string(), fmt_epoch(added_us / 1_000_000)),
            ];
            out.push(make_record(pairs, &dest_path, &self.common));
        }
        Ok(out)
    }
}

// ---- Google Spreadsheet（サービスアカウント JWT 認証）----
#[derive(serde::Serialize)]
struct GoogleJwtClaims {
    iss: String,
    scope: String,
    aud: String,
    exp: i64,
    iat: i64,
}

/// サービスアカウント JSON 鍵（の中身の文字列）から Google API アクセストークンを取得する。
fn google_access_token(sa_json: &str, scope: &str) -> Result<String> {
    let sa: Value = serde_json::from_str(sa_json)
        .map_err(|e| anyhow!("サービスアカウント JSON の解析に失敗: {e}"))?;
    let client_email = sa
        .get("client_email")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("client_email が見つかりません"))?;
    let private_key = sa
        .get("private_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("private_key が見つかりません"))?;
    let token_uri = sa
        .get("token_uri")
        .and_then(|v| v.as_str())
        .unwrap_or("https://oauth2.googleapis.com/token");

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;
    let claims = GoogleJwtClaims {
        iss: client_email.to_string(),
        scope: scope.to_string(),
        aud: token_uri.to_string(),
        exp: now + 3600,
        iat: now,
    };
    let key = jsonwebtoken::EncodingKey::from_rsa_pem(private_key.as_bytes())
        .map_err(|e| anyhow!("秘密鍵の読み込みに失敗: {e}"))?;
    let jwt = jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256),
        &claims,
        &key,
    )
    .map_err(|e| anyhow!("JWT 署名に失敗: {e}"))?;

    let resp = ureq::post(token_uri).send_form(&[
        ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
        ("assertion", &jwt),
    ]);
    let body: Value = match resp {
        Ok(r) => r.into_json()?,
        Err(ureq::Error::Status(code, r)) => {
            return Err(anyhow!(
                "Google トークン取得失敗 {code}: {}",
                r.into_string().unwrap_or_default()
            ))
        }
        Err(e) => return Err(anyhow!("Google 認証エンドポイントへの接続に失敗: {e}")),
    };
    body.get("access_token")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| anyhow!("access_token が返りませんでした"))
}

fn google_get_json(url: &str, token: &str) -> Result<Value> {
    let resp = ureq::get(url)
        .set("Authorization", &format!("Bearer {token}"))
        .call();
    match resp {
        Ok(r) => Ok(r.into_json()?),
        Err(ureq::Error::Status(code, r)) => Err(anyhow!(
            "Google API {code}: {}",
            r.into_string().unwrap_or_default()
        )),
        Err(e) => Err(anyhow!("Google API への接続に失敗: {e}")),
    }
}

#[derive(Deserialize)]
struct GoogleSheetsConnector {
    /// サービスアカウント JSON 鍵ファイルへのパス（ダウンロードした .json をそのまま指す）。
    key_path: String,
    spreadsheet_id: String,
    /// "Sheet1" や "Sheet1!A1:D50" 形式。省略時は先頭シートの全域。
    #[serde(default)]
    range: Option<String>,
    #[serde(default = "default_true")]
    has_header: bool,
    #[serde(flatten)]
    common: CommonCfg,
}

impl Connector for GoogleSheetsConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let sa_json = std::fs::read_to_string(&self.key_path)
            .map_err(|e| anyhow!("サービスアカウント鍵ファイルの読み込みに失敗: {e}"))?;
        let token = google_access_token(
            &sa_json,
            "https://www.googleapis.com/auth/spreadsheets.readonly",
        )?;

        let range = match &self.range {
            Some(r) if !r.is_empty() => r.clone(),
            _ => {
                let meta_url = format!(
                    "https://sheets.googleapis.com/v4/spreadsheets/{}?fields=sheets.properties.title",
                    self.spreadsheet_id
                );
                let meta = google_get_json(&meta_url, &token)?;
                meta.get("sheets")
                    .and_then(|s| s.as_array())
                    .and_then(|a| a.first())
                    .and_then(|s| s.get("properties"))
                    .and_then(|p| p.get("title"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        anyhow!("シート情報を取得できません（共有設定を確認してください）")
                    })?
                    .to_string()
            }
        };

        let values_url = format!(
            "https://sheets.googleapis.com/v4/spreadsheets/{}/values/{}",
            self.spreadsheet_id,
            urlencoding_light(&range)
        );
        let data = google_get_json(&values_url, &token)?;
        let rows_2d = data
            .get("values")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let cell_str = |v: &Value| -> String {
            match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => String::new(),
            }
        };

        let mut iter = rows_2d.iter();
        let headers: Vec<String> = if self.has_header {
            iter.next()
                .and_then(|r| r.as_array())
                .map(|r| r.iter().map(cell_str).collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let mut out = Vec::new();
        for row in iter {
            let cells = row.as_array().cloned().unwrap_or_default();
            if cells.is_empty() {
                continue;
            }
            let ncol = if self.has_header {
                headers.len().max(cells.len())
            } else {
                cells.len()
            };
            let pairs: Vec<(String, String)> = (0..ncol)
                .map(|c| {
                    let name = if self.has_header {
                        headers
                            .get(c)
                            .filter(|h| !h.is_empty())
                            .cloned()
                            .unwrap_or_else(|| col_to_letters(c as u32))
                    } else {
                        col_to_letters(c as u32)
                    };
                    let val = cells.get(c).map(cell_str).unwrap_or_default();
                    (name, val)
                })
                .collect();
            out.push(make_record(pairs, "", &self.common));
        }
        Ok(out)
    }
}

// ---- Google カレンダー / iCal（公開 .ics URL。認証不要）----
fn ical_unescape(s: &str) -> String {
    s.replace(r"\n", "\n")
        .replace(r"\N", "\n")
        .replace(r"\,", ",")
        .replace(r"\;", ";")
        .replace(r"\\", "\\")
}

/// "20260101T090000Z" / "20260101T090000" / "20260101"(終日) を表示用文字列へ。
fn parse_ical_datetime(v: &str) -> String {
    let digits: String = v
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == 'T')
        .collect();
    if digits.len() >= 15 && digits.as_bytes()[8] == b'T' {
        format!(
            "{}-{}-{} {}:{}:{}",
            &digits[0..4],
            &digits[4..6],
            &digits[6..8],
            &digits[9..11],
            &digits[11..13],
            &digits[13..15]
        )
    } else if digits.len() >= 8 {
        format!("{}-{}-{}", &digits[0..4], &digits[4..6], &digits[6..8])
    } else {
        v.to_string()
    }
}

/// RFC5545 の行折り返し(継続行は空白/タブで始まる)を1行に戻す。
fn ical_unfold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.replace("\r\n", "\n").lines() {
        if (line.starts_with(' ') || line.starts_with('\t')) && !out.is_empty() {
            out.push_str(line.trim_start_matches([' ', '\t']));
        } else {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(line);
        }
    }
    out
}

#[derive(Deserialize)]
struct IcalConnector {
    url: String,
    #[serde(flatten)]
    common: CommonCfg,
}

impl Connector for IcalConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let resp = ureq::get(&self.url).call();
        let text = match resp {
            Ok(r) => r.into_string()?,
            Err(ureq::Error::Status(code, r)) => {
                return Err(anyhow!(
                    "iCal 取得失敗 {code}: {}",
                    r.into_string().unwrap_or_default()
                ))
            }
            Err(e) => return Err(anyhow!("iCal URL への接続に失敗: {e}")),
        };
        let unfolded = ical_unfold(&text);

        let mut out = Vec::new();
        let mut cur: Option<std::collections::HashMap<String, String>> = None;
        for line in unfolded.lines() {
            match line {
                "BEGIN:VEVENT" => cur = Some(std::collections::HashMap::new()),
                "END:VEVENT" => {
                    if let Some(map) = cur.take() {
                        let summary = map.get("SUMMARY").cloned().unwrap_or_default();
                        let pairs = vec![
                            ("件名".to_string(), summary.clone()),
                            (
                                "開始".to_string(),
                                map.get("DTSTART").cloned().unwrap_or_default(),
                            ),
                            (
                                "終了".to_string(),
                                map.get("DTEND").cloned().unwrap_or_default(),
                            ),
                            (
                                "場所".to_string(),
                                map.get("LOCATION").cloned().unwrap_or_default(),
                            ),
                            (
                                "詳細".to_string(),
                                map.get("DESCRIPTION").cloned().unwrap_or_default(),
                            ),
                        ];
                        let mut rec = make_record(pairs, &self.url, &self.common);
                        if self.common.label_column.is_none() {
                            rec.label = Some(summary);
                        }
                        out.push(rec);
                    }
                }
                _ => {
                    if let (Some(map), Some(idx)) = (cur.as_mut(), line.find(':')) {
                        let key = line[..idx].split(';').next().unwrap_or("").to_uppercase();
                        let raw_value = ical_unescape(&line[idx + 1..]);
                        let value = if key == "DTSTART" || key == "DTEND" {
                            parse_ical_datetime(&raw_value)
                        } else {
                            raw_value
                        };
                        map.insert(key, value);
                    }
                }
            }
        }
        Ok(out)
    }
}

// ---- Airtable（REST API + Personal Access Token）----
fn airtable_value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(a) => a
            .iter()
            .map(|item| match item {
                Value::String(s) => s.clone(),
                Value::Object(o) => o
                    .get("filename")
                    .or_else(|| o.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                other => airtable_value_to_string(other),
            })
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    }
}

#[derive(Deserialize)]
struct AirtableConnector {
    token: String,
    base_id: String,
    /// テーブルの ID(tbl...) または名前。
    table: String,
    /// ビュー ID/名前（任意。指定するとそのビューの絞り込み/並び順が反映される）。
    #[serde(default)]
    view: Option<String>,
    #[serde(flatten)]
    common: CommonCfg,
}

impl Connector for AirtableConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let mut out = Vec::new();
        let mut offset: Option<String> = None;
        loop {
            let mut url = format!(
                "https://api.airtable.com/v0/{}/{}?pageSize=100",
                urlencoding_light(&self.base_id),
                urlencoding_light(&self.table)
            );
            if let Some(v) = &self.view {
                url.push_str(&format!("&view={}", urlencoding_light(v)));
            }
            if let Some(o) = &offset {
                url.push_str(&format!("&offset={}", urlencoding_light(o)));
            }
            let resp = ureq::get(&url)
                .set("Authorization", &format!("Bearer {}", self.token))
                .call();
            let body: Value = match resp {
                Ok(r) => r.into_json()?,
                Err(ureq::Error::Status(code, r)) => {
                    return Err(anyhow!(
                        "Airtable API エラー {code}: {}",
                        r.into_string().unwrap_or_default()
                    ))
                }
                Err(e) => return Err(anyhow!("Airtable への接続に失敗: {e}")),
            };
            if let Some(records) = body.get("records").and_then(|v| v.as_array()) {
                for rec in records {
                    let id = rec
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let mut pairs: Vec<(String, String)> = rec
                        .get("fields")
                        .and_then(|v| v.as_object())
                        .map(|o| {
                            o.iter()
                                .map(|(k, v)| (k.clone(), airtable_value_to_string(v)))
                                .collect()
                        })
                        .unwrap_or_default();
                    pairs.push(("レコードID".to_string(), id.clone()));
                    let open_ref = format!(
                        "https://airtable.com/{}/{}/{}",
                        self.base_id, self.table, id
                    );
                    out.push(make_record(pairs, &open_ref, &self.common));
                }
            }
            offset = body
                .get("offset")
                .and_then(|v| v.as_str())
                .map(String::from);
            if offset.is_none() {
                break;
            }
        }
        Ok(out)
    }
}

// ---- Box（OAuth 2.0。要件: クラウドストレージ系。§7-17 個人プランは Client Credentials
//      Grant が使えないため、access_token は commands::credentials::resolve_credentials 側で
//      Box にログイン済みの refresh_token から解決済みのものがここに渡ってくる）----
#[derive(Deserialize)]
struct BoxConnector {
    access_token: String,
    /// 走査開始フォルダ ID（省略時はルート "0"）。
    #[serde(default = "default_box_folder")]
    folder_id: String,
    #[serde(flatten)]
    common: CommonCfg,
}
fn default_box_folder() -> String {
    "0".to_string()
}

impl Connector for BoxConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let mut out = Vec::new();
        self.walk_box_folder(&self.access_token, &self.folder_id, "", &mut out)?;
        Ok(out)
    }
}

impl BoxConnector {
    fn walk_box_folder(
        &self,
        token: &str,
        folder_id: &str,
        path: &str,
        out: &mut Vec<Record>,
    ) -> Result<()> {
        let mut offset = 0u32;
        loop {
            let url = format!(
                "https://api.box.com/2.0/folders/{folder_id}/items?fields=name,type,size,modified_at,path_collection&limit=100&offset={offset}"
            );
            let resp = ureq::get(&url)
                .set("Authorization", &format!("Bearer {token}"))
                .call();
            let body: Value = match resp {
                Ok(r) => r.into_json()?,
                Err(ureq::Error::Status(code, r)) => {
                    return Err(anyhow!(
                        "Box API {code}: {}",
                        r.into_string().unwrap_or_default()
                    ))
                }
                Err(e) => return Err(anyhow!("Box への接続に失敗: {e}")),
            };
            let entries = body
                .get("entries")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            if entries.is_empty() {
                break;
            }
            for entry in &entries {
                let name = entry
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let item_type = entry.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let id = entry
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let size = entry
                    .get("size")
                    .map(json_scalar_to_string)
                    .unwrap_or_default();
                let modified = entry
                    .get("modified_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let web_url = if item_type == "folder" {
                    format!("https://app.box.com/folder/{id}")
                } else {
                    format!("https://app.box.com/file/{id}")
                };
                let pairs = vec![
                    ("名前".to_string(), name.clone()),
                    (
                        "種類".to_string(),
                        if item_type == "folder" {
                            "フォルダ"
                        } else {
                            "ファイル"
                        }
                        .to_string(),
                    ),
                    ("サイズ".to_string(), size),
                    ("更新日時".to_string(), modified),
                    ("パス".to_string(), path.to_string()),
                ];
                let mut rec = make_record(pairs, &web_url, &self.common);
                if self.common.label_column.is_none() {
                    rec.label = Some(name.clone());
                }
                out.push(rec);

                if item_type == "folder" {
                    let new_path = if path.is_empty() {
                        name.clone()
                    } else {
                        format!("{path}/{name}")
                    };
                    self.walk_box_folder(token, &id, &new_path, out)?;
                }
            }
            let total = body
                .get("total_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            offset += entries.len() as u32;
            if offset >= total {
                break;
            }
        }
        Ok(())
    }
}

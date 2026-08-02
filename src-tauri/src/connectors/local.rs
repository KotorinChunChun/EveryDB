//! ローカルファイル系connector（CSV / SQLite）。

use anyhow::{anyhow, Result};
use csv::ReaderBuilder;
use rusqlite::{Connection, OpenFlags};
use serde::Deserialize;
use serde_json::Value;
use std::io::Read;

use super::common::{make_record, make_record_with_row, CommonCfg};
use super::default_true;
use super::Connector;
use crate::model::Record;
// ---- CSV ----
#[derive(Deserialize)]
pub(crate) struct CsvConnector {
    path: String,
    #[serde(default = "default_true")]
    has_header: bool,
    /// 先頭から読み飛ばす行数（表のタイトル行・注記などを除外する。ヘッダー判定はこの後に行う）。
    #[serde(default)]
    skip_rows_top: u32,
    /// 末尾から読み飛ばす行数（合計行・脚注などを除外する）。
    #[serde(default)]
    skip_rows_bottom: u32,
    #[serde(flatten)]
    common: CommonCfg,
}

impl Connector for CsvConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        // 上下の読み飛ばしに対応するため、ヘッダー判定は自前で行う（csv 側は常に has_headers=false）。
        let mut rdr = ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_path(&self.path)?;

        // 末尾スキップのため全行をバッファする。
        let mut all: Vec<csv::StringRecord> = Vec::new();
        for result in rdr.records() {
            all.push(result?);
        }

        let start = (self.skip_rows_top as usize).min(all.len());
        let end = all
            .len()
            .saturating_sub(self.skip_rows_bottom as usize)
            .max(start);
        let mut iter = all[start..end].iter();

        let headers: Vec<String> = if self.has_header {
            match iter.next() {
                Some(first) => first
                    .iter()
                    .map(|s| s.trim_start_matches('\u{feff}').to_string())
                    .collect(),
                None => return Ok(Vec::new()),
            }
        } else {
            Vec::new()
        };

        let mut out = Vec::new();
        let mut row_idx = if self.has_header { 1i64 } else { 0i64 };
        for rec in iter {
            row_idx += 1;
            let pairs: Vec<(String, String)> = rec
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let name = headers
                        .get(i)
                        .cloned()
                        .unwrap_or_else(|| format!("col{}", i + 1));
                    (name, v.to_string())
                })
                .collect();
            out.push(make_record_with_row(
                pairs,
                &self.path,
                &self.common,
                Some(row_idx),
            ));
        }
        Ok(out)
    }

    fn field_names(&self) -> Result<Vec<String>> {
        let mut builder = csv::ReaderBuilder::new();
        builder.has_headers(self.has_header);
        let mut rdr = builder.from_path(&self.path)?;
        if self.has_header {
            let hdrs = rdr.headers()?;
            Ok(hdrs.iter().map(String::from).collect())
        } else {
            let mut iter = rdr.records();
            if let Some(r) = iter.next() {
                let rec = r?;
                Ok((0..rec.len()).map(|i| format!("col{}", i + 1)).collect())
            } else {
                Ok(Vec::new())
            }
        }
    }
}

// ---- SQLite ----
#[derive(Deserialize)]
pub(crate) struct SqliteConnector {
    path: String,
    table: String,
    #[serde(flatten)]
    common: CommonCfg,
}

impl Connector for SqliteConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        if !self.table.chars().all(|c| c.is_alphanumeric() || c == '_') || self.table.is_empty() {
            return Err(anyhow!("不正なテーブル名: {}", self.table));
        }
        let conn = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )?;
        let sql = format!("SELECT * FROM \"{}\"", self.table);
        let mut stmt = conn.prepare(&sql)?;
        let col_names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
        let ncol = col_names.len();

        let mut out = Vec::new();
        let mut rows = stmt.query([])?;
        let mut row_idx = 0i64;
        while let Some(r) = rows.next()? {
            row_idx += 1;
            let pairs: Vec<(String, String)> = (0..ncol)
                .map(|i| (col_names[i].clone(), value_ref_to_string(r.get_ref(i))))
                .collect();
            out.push(make_record_with_row(
                pairs,
                &self.path,
                &self.common,
                Some(row_idx),
            ));
        }
        Ok(out)
    }

    fn field_names(&self) -> Result<Vec<String>> {
        if !self.table.chars().all(|c| c.is_alphanumeric() || c == '_') || self.table.is_empty() {
            return Err(anyhow!("不正なテーブル名: {}", self.table));
        }
        let conn = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )?;
        let sql = format!("SELECT * FROM \"{}\" LIMIT 0", self.table);
        let stmt = conn.prepare(&sql)?;
        Ok(stmt.column_names().iter().map(|s| s.to_string()).collect())
    }
}

fn value_ref_to_string(v: rusqlite::Result<rusqlite::types::ValueRef>) -> String {
    use rusqlite::types::ValueRef;
    match v {
        Ok(ValueRef::Null) => String::new(),
        Ok(ValueRef::Integer(i)) => i.to_string(),
        Ok(ValueRef::Real(f)) => f.to_string(),
        Ok(ValueRef::Text(t)) => String::from_utf8_lossy(t).into_owned(),
        Ok(ValueRef::Blob(_)) => "[blob]".to_string(),
        Err(_) => String::new(),
    }
}

// ---- フォルダのファイルリスト ----
#[derive(Deserialize)]
pub(crate) struct FolderConnector {
    path: String,
    #[serde(default = "default_true")]
    recursive: bool,
    /// 拡張子フィルタ（小文字・ドット無し）。未指定なら全ファイル。
    #[serde(default)]
    extensions: Option<Vec<String>>,
}

impl Connector for FolderConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let max_depth = if self.recursive { usize::MAX } else { 1 };
        let exts: Option<Vec<String>> = self.extensions.as_ref().map(|v| {
            v.iter()
                .map(|e| e.trim_start_matches('.').to_lowercase())
                .collect()
        });

        let mut out = Vec::new();
        for entry in walkdir::WalkDir::new(&self.path)
            .max_depth(max_depth)
            .into_iter()
            .flatten()
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let p = entry.path();
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if let Some(ref filter) = exts {
                if !filter.contains(&ext) {
                    continue;
                }
            }
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let full = p.to_string_lossy().into_owned();
            let (size, modified) = match entry.metadata() {
                Ok(m) => {
                    let secs = m
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);
                    (m.len().to_string(), fmt_epoch(secs))
                }
                Err(_) => ("0".to_string(), String::new()),
            };
            let pairs = vec![
                ("名前".to_string(), name),
                ("パス".to_string(), full.clone()),
                ("拡張子".to_string(), ext),
                ("サイズ".to_string(), size),
                ("更新日時".to_string(), modified),
            ];
            // open_ref = ファイルの実パス。
            let cfg = CommonCfg {
                open_ref_column: Some("パス".to_string()),
                ..Default::default()
            };
            out.push(make_record(pairs, &full, &cfg));
        }
        Ok(out)
    }

    fn field_names(&self) -> Result<Vec<String>> {
        Ok(["名前", "パス", "拡張子", "サイズ", "更新日時"]
            .into_iter()
            .map(String::from)
            .collect())
    }
}

// ---- IME 辞書（エクスポート済みテキスト）----
#[derive(Deserialize)]
pub(crate) struct ImeCfg {
    path: String,
}

pub(crate) enum ImeFlavor {
    MsIme,  // UTF-16LE / 読み・語・品詞
    Google, // UTF-8   / 読み・単語・品詞・コメント
}

pub(crate) struct ImeConnector {
    cfg: ImeCfg,
    flavor: ImeFlavor,
}

impl ImeConnector {
    pub(crate) fn new(cfg: ImeCfg, flavor: ImeFlavor) -> Self {
        Self { cfg, flavor }
    }
}

impl Connector for ImeConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let bytes = std::fs::read(&self.cfg.path)?;
        let (text, columns): (std::borrow::Cow<str>, &[&str]) = match self.flavor {
            ImeFlavor::MsIme => {
                let (t, _, _) = encoding_rs::UTF_16LE.decode(&bytes);
                (t, &["読み", "語", "品詞"])
            }
            ImeFlavor::Google => {
                let (t, _, _) = encoding_rs::UTF_8.decode(&bytes);
                (t, &["読み", "単語", "品詞", "コメント"])
            }
        };

        let mut out = Vec::new();
        for line in text.lines() {
            let line = line.trim_end_matches('\r');
            if line.is_empty() || line.starts_with('!') {
                continue; // 空行・コメント行(! 始まり)は無視
            }
            let cols: Vec<&str> = line.split('\t').collect();
            let pairs: Vec<(String, String)> = columns
                .iter()
                .enumerate()
                .map(|(i, name)| (name.to_string(), cols.get(i).unwrap_or(&"").to_string()))
                .collect();
            // label = 2 列目(語/単語)、open_ref = 辞書ファイルパス。
            let cfg = CommonCfg {
                label_column: Some(columns[1].to_string()),
                ..Default::default()
            };
            out.push(make_record(pairs, &self.cfg.path, &cfg));
        }
        Ok(out)
    }

    fn field_names(&self) -> Result<Vec<String>> {
        let names: &[&str] = match self.flavor {
            ImeFlavor::MsIme => &["読み", "語", "品詞"],
            ImeFlavor::Google => &["読み", "単語", "品詞", "コメント"],
        };
        Ok(names.iter().map(|name| (*name).to_string()).collect())
    }
}

// ---- Access (accdb / mdb) : ODBC 経由（要件 M4）----
#[derive(Deserialize)]
pub(crate) struct AccessConnector {
    path: String,
    table: String,
    #[serde(flatten)]
    common: CommonCfg,
}

/// `[table]` で囲む前提の識別子検証。閉じ括弧・セミコロン等の危険文字のみ拒否
/// （Access はテーブル名に日本語・空白を許すため英数字限定にはしない）。
fn valid_bracket_ident(s: &str) -> bool {
    !s.is_empty()
        && !s
            .chars()
            .any(|c| c == ']' || c == '[' || c == ';' || c.is_control())
}

impl Connector for AccessConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        use odbc_api::{ConnectionOptions, Cursor, Environment, ResultSetMetadata};

        if !valid_bracket_ident(&self.table) {
            return Err(anyhow!("不正なテーブル名: {}", self.table));
        }
        // ODBC 接続文字列は `;` が属性の区切りなので、パスに `;` が混ざると
        // 後続に任意の接続属性を注入できてしまう。ここで弾く。
        if self.path.contains(';') || self.path.contains('{') || self.path.contains('}') {
            return Err(anyhow!(
                "パスに ODBC 接続文字列で使えない文字（; {{ }}）が含まれています: {}",
                self.path
            ));
        }
        // ODBC ドライバマネージャ経由で ACE ドライバに接続。
        let cs = format!(
            "Driver={{Microsoft Access Driver (*.mdb, *.accdb)}};Dbq={};",
            self.path
        );
        let env = Environment::new().map_err(|e| anyhow!("ODBC 環境の初期化に失敗: {e}"))?;
        let conn = env
            .connect_with_connection_string(&cs, ConnectionOptions::default())
            .map_err(|e| anyhow!("Access への接続に失敗（ドライバ/パス確認）: {e}"))?;

        let sql = format!("SELECT * FROM [{}]", self.table);
        let mut cursor = match conn.execute(&sql, (), None)? {
            Some(c) => c,
            None => return Ok(Vec::new()),
        };

        let ncol = cursor.num_result_cols()? as u16;
        let mut names = Vec::with_capacity(ncol as usize);
        for i in 1..=ncol {
            names.push(cursor.col_name(i)?);
        }

        let mut out = Vec::new();
        let mut buf: Vec<u16> = Vec::new();
        while let Some(mut row) = cursor.next_row()? {
            let mut pairs = Vec::with_capacity(ncol as usize);
            for i in 1..=ncol {
                buf.clear();
                // UTF-16(ワイド)で取得 → ロケール非依存で全 Unicode を安全に扱える。
                let not_null = row.get_wide_text(i, &mut buf)?;
                let val = if not_null {
                    String::from_utf16_lossy(&buf)
                        .trim_end_matches('\u{0}')
                        .to_string()
                } else {
                    String::new()
                };
                pairs.push((names[(i - 1) as usize].clone(), val));
            }
            out.push(make_record(pairs, &self.path, &self.common));
        }
        Ok(out)
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
pub(crate) fn webkit_micros_to_unix_secs(us: i64) -> i64 {
    us / 1_000_000 - 11_644_473_600
}

const HISTORY_ROW_CAP: usize = 10_000;

#[derive(Deserialize)]
pub(crate) struct BrowserHistoryConnector {
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

    fn field_names(&self) -> Result<Vec<String>> {
        Ok(["タイトル", "URL", "訪問回数", "最終訪問"]
            .into_iter()
            .map(String::from)
            .collect())
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
pub(crate) struct BrowserBookmarksConnector {
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

    fn field_names(&self) -> Result<Vec<String>> {
        let names: &[&str] = if self.browser == "firefox" {
            &["名前", "URL", "追加日時"]
        } else {
            &["名前", "URL", "フォルダ", "追加日時"]
        };
        Ok(names.iter().map(|name| (*name).to_string()).collect())
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
pub(crate) fn basename_of(path: &str) -> String {
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
pub(crate) fn file_uri_to_path(uri: &str) -> String {
    let decoded = percent_decode(uri);
    let stripped = decoded
        .strip_prefix("file:///")
        .or_else(|| decoded.strip_prefix("file://"))
        .unwrap_or(&decoded);
    stripped.replace('/', "\\")
}

#[derive(Deserialize)]
pub(crate) struct BrowserDownloadsConnector {
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

    fn field_names(&self) -> Result<Vec<String>> {
        let names: &[&str] = if self.browser == "firefox" {
            &["ファイル名", "保存先パス", "URL", "タイトル", "追加日時"]
        } else {
            &[
                "ファイル名",
                "保存先パス",
                "URL",
                "状態",
                "サイズ(バイト)",
                "開始日時",
                "完了日時",
            ]
        };
        Ok(names.iter().map(|name| (*name).to_string()).collect())
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
pub(crate) struct ExcelTableConnector {
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
pub(crate) fn col_to_letters(mut col: u32) -> String {
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
pub(crate) fn parse_ref_columns(s: &str) -> Result<Vec<u32>> {
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
pub(crate) struct ExcelRangeConnector {
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
pub(crate) struct ExcelSheetsConnector {
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

    fn field_names(&self) -> Result<Vec<String>> {
        Ok(vec!["シート名".to_string()])
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

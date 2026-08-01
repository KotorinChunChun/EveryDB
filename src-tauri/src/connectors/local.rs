//! ローカルファイル系connector（CSV / SQLite）。

use anyhow::{anyhow, Result};
use csv::ReaderBuilder;
use rusqlite::{Connection, OpenFlags};
use serde::Deserialize;
use serde_json::Value;

use super::common::{make_record, make_record_with_row, CommonCfg};
use super::default_true;
use super::fmt_epoch;
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

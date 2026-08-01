//! ローカルファイル系connector（CSV / SQLite）。

use anyhow::{anyhow, Result};
use csv::ReaderBuilder;
use rusqlite::{Connection, OpenFlags};
use serde::Deserialize;

use super::common::{make_record_with_row, CommonCfg};
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

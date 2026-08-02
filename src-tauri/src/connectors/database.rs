//! 外部データベースconnector（PostgreSQL / MySQL）。

use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::Value;

use super::common::{make_record, CommonCfg};
use super::limits::DB_FETCH_LIMIT;
use super::{json_scalar_to_string, urlencoding_light, Connector};
use crate::model::Record;
/// SQL 識別子として安全か（テーブル名/スキーマ名の簡易検証。英数字・アンダースコアのみ許可）。
fn is_safe_sql_ident(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// ポート番号を「数値」でも「文字列」でも受け取れる型（フロントは text 入力のため文字列で来る）。
#[derive(Deserialize)]
#[serde(untagged)]
pub(crate) enum FlexPort {
    Num(u16),
    Str(String),
}
impl FlexPort {
    fn to_u16(&self, default: u16) -> u16 {
        match self {
            FlexPort::Num(n) => *n,
            FlexPort::Str(s) => s.trim().parse().unwrap_or(default),
        }
    }
}

// ---- PostgreSQL（要件: 外部 DB コネクタ系）----
#[derive(Deserialize)]
pub(crate) struct PostgresConnector {
    host: String,
    #[serde(default)]
    port: Option<FlexPort>,
    database: String,
    user: String,
    password: String,
    table: String,
    #[serde(default)]
    schema: Option<String>,
    /// libpq 互換の sslmode（disable / prefer / require / verify-ca / verify-full）。既定 prefer。
    #[serde(default)]
    sslmode: Option<String>,
    /// 自己署名証明書の社内サーバー向け。既定 false（＝証明書を検証する）。
    #[serde(default)]
    ssl_accept_invalid_certs: Option<bool>,
    #[serde(flatten)]
    common: CommonCfg,
}

/// libpq のキーワード/値形式の値をクォートする（`\` と `'` をエスケープして `'…'` で囲む）。
fn pg_quote(v: &str) -> String {
    let escaped = v.replace('\\', r"\\").replace('\'', r"\'");
    format!("'{escaped}'")
}

impl PostgresConnector {
    fn schema_name(&self) -> Result<&str> {
        let schema = self.schema.as_deref().unwrap_or("public");
        if !is_safe_sql_ident(&self.table) || !is_safe_sql_ident(schema) {
            return Err(anyhow!(
                "不正なテーブル名/スキーマ名: {}.{}",
                schema,
                self.table
            ));
        }
        Ok(schema)
    }

    fn connect(&self) -> Result<postgres::Client> {
        let port = self.port.as_ref().map(|p| p.to_u16(5432)).unwrap_or(5432);
        // 値は必ずクォート＋エスケープする。素の format! だと空白入りパスワード等で
        // キーワード/値の区切りが壊れ、後続に別パラメータ（host= 等）を注入されうる。
        let conn_str = format!(
            "host={} port={} dbname={} user={} password={} connect_timeout=10 sslmode={}",
            pg_quote(&self.host),
            port,
            pg_quote(&self.database),
            pg_quote(&self.user),
            pg_quote(&self.password),
            self.sslmode.as_deref().unwrap_or("prefer"),
        );
        // 平文接続を既定にしない。sslmode=prefer なので、TLS 非対応サーバーへは
        // 従来どおり平文でフォールバックする（既存の接続設定を壊さない）。
        let tls = native_tls::TlsConnector::builder()
            .danger_accept_invalid_certs(self.ssl_accept_invalid_certs.unwrap_or(false))
            .build()
            .map_err(|e| anyhow!("TLS の初期化に失敗: {e}"))?;
        postgres::Client::connect(&conn_str, postgres_native_tls::MakeTlsConnector::new(tls))
            .map_err(|e| anyhow!("PostgreSQL への接続に失敗: {e}"))
    }
}

impl Connector for PostgresConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let schema = self.schema_name()?;
        let mut client = self.connect()?;
        // row_to_json で行ごと JSON 化して受け取ることで、列の型ごとの分岐が不要になる。
        let sql = format!(
            "SELECT row_to_json(t) FROM \"{schema}\".\"{}\" t LIMIT {}",
            self.table, DB_FETCH_LIMIT
        );
        let rows = client
            .query(&sql, &[])
            .map_err(|e| anyhow!("クエリ実行に失敗: {e}"))?;
        let mut out = Vec::new();
        for row in &rows {
            let json_val: Value = row.get(0);
            if let Some(obj) = json_val.as_object() {
                let pairs: Vec<(String, String)> = obj
                    .iter()
                    .map(|(k, v)| (k.clone(), json_scalar_to_string(v)))
                    .collect();
                out.push(make_record(pairs, "", &self.common));
            }
        }
        Ok(out)
    }

    fn field_names(&self) -> Result<Vec<String>> {
        let schema = self.schema_name()?;
        let mut client = self.connect()?;
        let sql = format!("SELECT * FROM \"{schema}\".\"{}\" LIMIT 0", self.table);
        let statement = client
            .prepare(&sql)
            .map_err(|e| anyhow!("列情報の取得に失敗: {e}"))?;
        Ok(statement
            .columns()
            .iter()
            .map(|column| column.name().to_string())
            .collect())
    }
}

// ---- MySQL（要件: 外部 DB コネクタ系）----
#[derive(Deserialize)]
pub(crate) struct MysqlConnector {
    host: String,
    #[serde(default)]
    port: Option<FlexPort>,
    database: String,
    user: String,
    password: String,
    table: String,
    /// TLS で接続する。既定 false（従来の平文接続との互換のため）。
    /// リモートの MySQL を参照する場合は有効化すること。
    #[serde(default)]
    use_tls: Option<bool>,
    /// 自己署名証明書の社内サーバー向け。既定 false（＝証明書を検証する）。
    #[serde(default)]
    ssl_accept_invalid_certs: Option<bool>,
    #[serde(flatten)]
    common: CommonCfg,
}

fn mysql_value_to_string(v: &mysql::Value) -> String {
    use mysql::Value as V;
    match v {
        V::NULL => String::new(),
        V::Bytes(b) => String::from_utf8_lossy(b).into_owned(),
        V::Int(i) => i.to_string(),
        V::UInt(u) => u.to_string(),
        V::Float(f) => f.to_string(),
        V::Double(d) => d.to_string(),
        V::Date(y, mo, d, h, mi, s, us) => {
            if *us == 0 {
                format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}")
            } else {
                format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}.{us:06}")
            }
        }
        V::Time(neg, days, h, mi, s, us) => {
            let total_h = *days as u64 * 24 + *h as u64;
            let sign = if *neg { "-" } else { "" };
            if *us == 0 {
                format!("{sign}{total_h:02}:{mi:02}:{s:02}")
            } else {
                format!("{sign}{total_h:02}:{mi:02}:{s:02}.{us:06}")
            }
        }
    }
}

impl MysqlConnector {
    fn options(&self) -> Result<mysql::OptsBuilder> {
        let port = self.port.as_ref().map(|p| p.to_u16(3306)).unwrap_or(3306);
        let url = format!(
            "mysql://{}:{}@{}:{}/{}",
            urlencoding_light(&self.user),
            urlencoding_light(&self.password),
            self.host,
            port,
            urlencoding_light(&self.database)
        );
        let mut opts = mysql::OptsBuilder::from_opts(
            mysql::Opts::from_url(url.as_str())
                .map_err(|e| anyhow!("MySQL の接続 URL が不正: {e}"))?,
        );
        if self.use_tls.unwrap_or(false) {
            opts = opts.ssl_opts(Some(
                mysql::SslOpts::default().with_danger_accept_invalid_certs(
                    self.ssl_accept_invalid_certs.unwrap_or(false),
                ),
            ));
        }
        Ok(opts)
    }
}

impl Connector for MysqlConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        use mysql::prelude::Queryable;
        if !is_safe_sql_ident(&self.table) {
            return Err(anyhow!("不正なテーブル名: {}", self.table));
        }
        let pool =
            mysql::Pool::new(self.options()?).map_err(|e| anyhow!("MySQL への接続に失敗: {e}"))?;
        let mut conn = pool
            .get_conn()
            .map_err(|e| anyhow!("MySQL 接続の確立に失敗: {e}"))?;
        let sql = format!("SELECT * FROM `{}` LIMIT {}", self.table, DB_FETCH_LIMIT);
        let result = conn
            .query_iter(&sql)
            .map_err(|e| anyhow!("クエリ実行に失敗: {e}"))?;
        let col_names: Vec<String> = result
            .columns()
            .as_ref()
            .iter()
            .map(|c| c.name_str().to_string())
            .collect();

        let mut out = Vec::new();
        for row_result in result {
            let row = row_result.map_err(|e| anyhow!("行の読み取りに失敗: {e}"))?;
            let pairs: Vec<(String, String)> = (0..col_names.len())
                .map(|i| {
                    let val = row.as_ref(i).map(mysql_value_to_string).unwrap_or_default();
                    (col_names[i].clone(), val)
                })
                .collect();
            out.push(make_record(pairs, "", &self.common));
        }
        Ok(out)
    }

    fn field_names(&self) -> Result<Vec<String>> {
        use mysql::prelude::Queryable;
        if !is_safe_sql_ident(&self.table) {
            return Err(anyhow!("不正なテーブル名: {}", self.table));
        }
        let pool =
            mysql::Pool::new(self.options()?).map_err(|e| anyhow!("MySQL への接続に失敗: {e}"))?;
        let mut conn = pool
            .get_conn()
            .map_err(|e| anyhow!("MySQL 接続の確立に失敗: {e}"))?;
        let sql = format!("SELECT * FROM `{}` LIMIT 0", self.table);
        let result = conn
            .query_iter(&sql)
            .map_err(|e| anyhow!("列情報の取得に失敗: {e}"))?;
        Ok(result
            .columns()
            .as_ref()
            .iter()
            .map(|column| column.name_str().to_string())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{is_safe_sql_ident, pg_quote};

    #[test]
    fn sql_identifier_allowlist_rejects_injection_characters() {
        assert!(is_safe_sql_ident("valid_123"));
        assert!(!is_safe_sql_ident(""));
        assert!(!is_safe_sql_ident("users; DROP TABLE users"));
        assert!(!is_safe_sql_ident("schema.table"));
    }

    #[test]
    fn postgres_connection_values_are_quoted() {
        assert_eq!(pg_quote("pass word"), "'pass word'");
        assert_eq!(pg_quote("a'b\\c"), r"'a\'b\\c'");
    }
}

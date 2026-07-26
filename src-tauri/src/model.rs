use serde::Serialize;
use serde_json::Map;

/// 全データソースを正規化した共通レコード（要件 §5）。
/// fields は挿入順を保持する（serde_json の preserve_order を利用）。
pub struct Record {
    pub label: Option<String>,
    pub open_ref: Option<String>,
    pub row_number: Option<i64>,
    pub fields: Map<String, serde_json::Value>,
    /// searchable フィールドを連結した検索専用テキスト。
    pub search_text: String,
}

/// UI 用のソース情報（一覧・ステータス表示）。
#[derive(Serialize)]
pub struct SourceInfo {
    pub id: i64,
    #[serde(rename = "type_")]
    pub type_: String,
    pub name: String,
    pub refresh_interval: i64,
    pub status: Option<String>,
    pub last_success_at: Option<String>,
    pub last_error: Option<String>,
    pub record_count: i64,
    /// ツリー表示用の親キー（ファイルパス / サイト URL / Notion DB / "Microsoft 365" 等）。
    pub parent: String,
}

/// 検索結果の 1 行。
#[derive(Serialize)]
pub struct Row {
    pub source_id: i64,
    pub source_name: String,
    pub label: Option<String>,
    pub open_ref: Option<String>,
    pub row_number: Option<i64>,
    pub fields: Map<String, serde_json::Value>,
}

/// 検索レスポンス（seq で最新のみ採用させる）。
#[derive(Serialize)]
pub struct SearchResponse {
    pub seq: i64,
    pub rows: Vec<Row>,
    pub truncated: bool,
}

/// 検索プリセット/お気に入り（要件 REQ-BM-01）。
#[derive(Serialize)]
pub struct SearchPreset {
    pub id: i64,
    pub name: String,
    pub query: String,
    pub source_ids: Vec<i64>,
    pub created_at: String,
}

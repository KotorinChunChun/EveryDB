//! connector共通のフィールド役割設定とRecord変換。

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::model::Record;

/// フィールド役割の共通設定（要件 §5-1 / §9-3）。未指定なら「全列 display+searchable /
/// 先頭列 label / open_ref=パス」。
#[derive(Deserialize, Default)]
pub(crate) struct CommonCfg {
    #[serde(default)]
    pub(crate) label_column: Option<String>,
    #[serde(default)]
    pub(crate) open_ref_column: Option<String>,
    #[serde(default)]
    pub(crate) searchable_columns: Option<Vec<String>>,
    /// 表示する列（要件 §9-3: 列 ON/OFF）。None = 全列表示。
    #[serde(default)]
    pub(crate) display_columns: Option<Vec<String>>,
    /// 表示名エイリアス: 元の列名 → 表示名（要件 §9-3: 列統合表示）。
    /// 複数ソースで同じエイリアスを付けると、結果表で同一列として自動統合される
    /// （フロント側の列合成は既存の「列名の和集合」ロジックがそのまま機能する）。
    #[serde(default)]
    pub(crate) column_aliases: Option<std::collections::HashMap<String, String>>,
    /// 列の表示順（元の列名の並び。ドラッグ並べ替え用）。None = 検出順のまま。
    #[serde(default)]
    pub(crate) column_order: Option<Vec<String>>,
}

pub(crate) fn make_record_with_row(
    pairs: Vec<(String, String)>,
    default_open_ref: &str,
    cfg: &CommonCfg,
    row_number: Option<i64>,
) -> Record {
    // label / open_ref / 検索テキストは「表示 ON/OFF」の影響を受けない、元の全フィールドから計算する
    // （検索対象列は searchable_columns で別軸に制御するため。要件 §9-3）。
    let find = |col: &str| pairs.iter().find(|(k, _)| k == col).map(|(_, v)| v.clone());

    let label = match &cfg.label_column {
        Some(c) => find(c),
        None => pairs.first().map(|(_, v)| v.clone()),
    };
    let open_ref = match &cfg.open_ref_column {
        Some(c) => find(c),
        None => Some(default_open_ref.to_string()),
    };
    let search_text = match &cfg.searchable_columns {
        Some(cols) => pairs
            .iter()
            .filter(|(k, _)| cols.contains(k))
            .map(|(_, v)| v.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        None => pairs
            .iter()
            .map(|(_, v)| v.as_str())
            .collect::<Vec<_>>()
            .join(" "),
    };

    // 表示対象の絞り込み(display_columns) + 表示順(column_order) を適用する順序で pairs を並べ替えてから
    // 表示名エイリアス(column_aliases) を適用して fields を組む（要件: 列のドラッグ並べ替え）。
    let ordered_pairs: Vec<&(String, String)> = match &cfg.column_order {
        Some(order) => {
            let mut ordered: Vec<&(String, String)> = order
                .iter()
                .filter_map(|name| pairs.iter().find(|(k, _)| k == name))
                .collect();
            for p in &pairs {
                if !order.contains(&p.0) {
                    ordered.push(p);
                }
            }
            ordered
        }
        None => pairs.iter().collect(),
    };

    let mut fields = Map::new();
    for (k, v) in ordered_pairs {
        let show = cfg
            .display_columns
            .as_ref()
            .map(|cols| cols.contains(k))
            .unwrap_or(true);
        if !show {
            continue;
        }
        let key = cfg
            .column_aliases
            .as_ref()
            .and_then(|m| m.get(k))
            .cloned()
            .unwrap_or_else(|| k.clone());
        fields.insert(key, Value::String(v.clone()));
    }
    Record {
        label,
        open_ref,
        row_number,
        fields,
        search_text,
    }
}

pub(crate) fn make_record(
    pairs: Vec<(String, String)>,
    default_open_ref: &str,
    cfg: &CommonCfg,
) -> Record {
    make_record_with_row(pairs, default_open_ref, cfg, None)
}

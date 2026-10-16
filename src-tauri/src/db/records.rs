use anyhow::Result;
use rusqlite::{params, Connection};

use crate::model::Record;
/// スナップショット全置換（要件 §4-1）。トランザクション内で全削除 → 再挿入。
pub fn replace_source_records(
    conn: &mut Connection,
    source_id: i64,
    records: &[Record],
) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM records WHERE source_id = ?1", [source_id])?;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO records(source_id, label, open_ref, row_number, fields_json, search_text)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for r in records {
            let fields_json = serde_json::to_string(&r.fields)?;
            stmt.execute(params![
                source_id,
                r.label,
                r.open_ref,
                r.row_number,
                fields_json,
                r.search_text
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

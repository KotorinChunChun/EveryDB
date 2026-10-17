//! RF-50-08: オフライン性能ベースライン（開発者向け計測）。

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use rusqlite::Connection;
    use serde_json::json;

    use crate::app_state::AppState;
    use crate::db;
    use crate::model::Record;
    use crate::refresh_service;

    fn bench_record(i: usize) -> Record {
        let name = format!("user{i:05}");
        Record {
            label: Some(name.clone()),
            open_ref: Some(format!("file:///C:/data/{name}.txt")),
            row_number: Some(i as i64),
            fields: serde_json::from_value(json!({
                "name": name,
                "department": "営業部",
                "city": "東京都",
            }))
            .unwrap(),
            search_text: format!("{name} 営業部 東京都"),
        }
    }

    fn seed_source(conn: &mut Connection, rows: usize) -> i64 {
        let sid = db::insert_source(conn, "csv", "bench", "{}").unwrap();
        let records: Vec<Record> = (0..rows).map(bench_record).collect();
        db::replace_source_records(conn, sid, &records).unwrap();
        conn.execute(
            "INSERT INTO records_fts(records_fts) VALUES('optimize')",
            [],
        )
        .unwrap();
        sid
    }

    fn run_search_baseline(label: &str, rows: usize) -> Duration {
        let mut conn = Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        let sid = seed_source(&mut conn, rows);
        let started = Instant::now();
        let (found, truncated) = db::search(&conn, "user00042", &[sid], 10_000).unwrap();
        let elapsed = started.elapsed();
        assert!(!found.is_empty(), "{label}: expected at least one hit");
        assert!(!truncated, "{label}: should not truncate at 10k limit");
        eprintln!("RF-50-08 {label}: {rows} rows seeded, search took {elapsed:?}");
        elapsed
    }

    #[test]
    fn baseline_search_10k_rows() {
        let elapsed = run_search_baseline("search_10k", 10_000);
        assert!(
            elapsed < Duration::from_secs(3),
            "10k search baseline exceeded 3s: {elapsed:?}"
        );
    }

    #[test]
    fn baseline_search_100k_rows() {
        let mut conn = Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        let sid = seed_source(&mut conn, 100_000);
        let started = Instant::now();
        let (found, truncated) =
            db::search(&conn, "user00042 営業部 東京都", &[sid], 10_000).unwrap();
        let elapsed = started.elapsed();
        assert_eq!(found.len(), 1);
        assert!(!truncated);
        eprintln!("v0.1.0 search_100k_three_terms: search took {elapsed:?}");
        assert!(
            elapsed < Duration::from_millis(500),
            "100k three-term search baseline exceeded 500ms: {elapsed:?}"
        );
    }

    #[test]
    fn baseline_reload_100_sources() {
        let conn = Connection::open_in_memory().unwrap();
        db::init_schema(&conn).unwrap();
        let state = AppState::new(conn);
        let samples = crate::test_fixtures::sample("customers.csv");
        let cfg = json!({ "path": samples.to_string_lossy() }).to_string();
        let mut ids = Vec::with_capacity(100);
        for i in 0..100 {
            let id = db::insert_source(
                &state.db.lock().unwrap(),
                "csv",
                &format!("bench-{i:03}"),
                &cfg,
            )
            .unwrap();
            ids.push(id);
        }
        let started = Instant::now();
        let summary = refresh_service::reload_many_for_state(&state, &ids);
        let elapsed = started.elapsed();
        eprintln!(
            "RF-50-08 reload_100_sources: {} sources, {} ok / {} failed, took {elapsed:?}",
            ids.len(),
            summary.successful,
            summary.failed
        );
        assert_eq!(summary.results.len(), 100);
        assert!(
            summary.successful >= 1,
            "at least one source should reload from sample csv"
        );
        assert!(
            elapsed < Duration::from_secs(120),
            "100-source reload baseline exceeded 120s: {elapsed:?}"
        );
    }
}

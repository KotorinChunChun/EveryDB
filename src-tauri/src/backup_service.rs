//! SQLite Backup APIによる書き出しと、次回起動時の安全な復元。

use anyhow::{anyhow, Context, Result};
use rusqlite::{backup::Backup, Connection, OpenFlags};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::app_state::AppState;
use crate::db::CURRENT_SCHEMA_VERSION;
use crate::service_error::lock_db;

const STAGED_FILE: &str = "cache.db.restore-staged";
const PENDING_FILE: &str = "restore-pending";

#[derive(Debug, Clone, Serialize)]
pub struct BackupInfo {
    pub schema_version: i64,
    pub source_count: i64,
}

pub fn export_backup(state: &AppState, destination: &Path) -> Result<BackupInfo> {
    let conn = lock_db(state).map_err(|error| anyhow!(error.to_string()))?;
    write_consistent_backup(&conn, destination)?;
    validate_backup(destination)
}

fn write_consistent_backup(source: &Connection, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        if !parent.is_dir() {
            return Err(anyhow!(
                "バックアップ先フォルダーが存在しません: {}",
                parent.display()
            ));
        }
    }
    let temp = destination.with_extension(format!("tmp-{}", std::process::id()));
    if temp.exists() {
        fs::remove_file(&temp)?;
    }
    let mut output = Connection::open(&temp)?;
    {
        let backup = Backup::new(source, &mut output)?;
        backup.run_to_completion(64, Duration::from_millis(10), None)?;
    }
    drop(output);
    validate_backup(&temp)?;

    let previous = destination.with_extension(format!("previous-{}", timestamp()));
    let had_previous = destination.exists();
    if had_previous {
        fs::rename(destination, &previous)?;
    }
    if let Err(error) = fs::rename(&temp, destination) {
        if had_previous {
            let _ = fs::rename(&previous, destination);
        }
        return Err(error).context("バックアップファイルの確定に失敗しました");
    }
    if had_previous {
        fs::remove_file(previous)?;
    }
    Ok(())
}

pub fn validate_backup(path: &Path) -> Result<BackupInfo> {
    if !path.is_file() {
        return Err(anyhow!("SQLiteバックアップファイルが見つかりません"));
    }
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .context("SQLiteバックアップとして開けません")?;
    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(anyhow!("SQLite整合性検査に失敗しました: {integrity}"));
    }
    for table in ["app_meta", "sources", "records"] {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
            [table],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(anyhow!(
                "EveryDBバックアップではありません（{table}がありません）"
            ));
        }
    }
    let version_text: String = conn.query_row(
        "SELECT value FROM app_meta WHERE key='schema_version'",
        [],
        |row| row.get(0),
    )?;
    let schema_version = version_text
        .parse::<i64>()
        .context("schema versionが不正です")?;
    if schema_version > CURRENT_SCHEMA_VERSION {
        return Err(anyhow!(
            "このバックアップは新しいEveryDB用です（schema {schema_version}、対応 {CURRENT_SCHEMA_VERSION}）"
        ));
    }
    let source_count = conn.query_row("SELECT COUNT(*) FROM sources", [], |row| row.get(0))?;
    Ok(BackupInfo {
        schema_version,
        source_count,
    })
}

pub fn stage_restore(app_data_dir: &Path, source: &Path) -> Result<BackupInfo> {
    let info = validate_backup(source)?;
    fs::create_dir_all(app_data_dir)?;
    let staged = app_data_dir.join(STAGED_FILE);
    let pending = app_data_dir.join(PENDING_FILE);
    if staged.exists() {
        fs::remove_file(&staged)?;
    }
    fs::copy(source, &staged).context("復元ファイルを一時領域へコピーできません")?;
    validate_backup(&staged)?;
    fs::write(&pending, b"pending").context("復元予約を保存できません")?;
    Ok(info)
}

pub fn apply_pending_restore(app_data_dir: &Path) -> Result<Option<PathBuf>> {
    let pending = app_data_dir.join(PENDING_FILE);
    if !pending.exists() {
        return Ok(None);
    }
    let staged = app_data_dir.join(STAGED_FILE);
    validate_backup(&staged).context("ステージ済み復元ファイルが不正です")?;

    let live = app_data_dir.join("cache.db");
    let rollback = app_data_dir.join(format!("cache.db.before-restore-{}.bak", timestamp()));
    let had_live = live.exists();
    if had_live {
        let current = Connection::open(&live)?;
        write_consistent_backup(&current, &rollback)
            .context("復元前バックアップを作成できません")?;
        drop(current);
    }

    remove_if_exists(&live.with_extension("db-wal"))?;
    remove_if_exists(&live.with_extension("db-shm"))?;
    remove_if_exists(&live)?;
    if let Err(error) = fs::rename(&staged, &live) {
        restore_rollback(&rollback, &live)?;
        remove_if_exists(&pending)?;
        return Err(error).context("ステージ済みDBを配置できません");
    }

    if let Err(error) = validate_backup(&live) {
        remove_if_exists(&live)?;
        restore_rollback(&rollback, &live)?;
        remove_if_exists(&pending)?;
        return Err(error).context("配置後のDB検証に失敗したため元へ戻しました");
    }
    remove_if_exists(&pending)?;
    Ok(had_live.then_some(rollback))
}

fn restore_rollback(rollback: &Path, live: &Path) -> Result<()> {
    if rollback.exists() {
        fs::rename(rollback, live).context("元のDBへの復旧に失敗しました")?;
    }
    Ok(())
}

fn remove_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("everydb-backup-{name}-{}", timestamp()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn create_database(path: &Path, name: &str) {
        let conn = db::open_cached_database(path).unwrap();
        db::insert_source(
            &conn,
            "notion",
            name,
            r#"{"token":"dpapi:encrypted-value","database_id":"db"}"#,
        )
        .unwrap();
    }

    #[test]
    fn export_is_consistent_and_keeps_only_encrypted_secret() {
        let dir = temp_dir("export");
        let live = dir.join("cache.db");
        create_database(&live, "original");
        let state = AppState::new(Connection::open(&live).unwrap());
        let output = dir.join("backup.sqlite3");

        let info = export_backup(&state, &output).unwrap();

        assert_eq!(info.source_count, 1);
        let bytes = fs::read(&output).unwrap();
        let dump = String::from_utf8_lossy(&bytes);
        assert!(!dump.contains("plain-secret"));
        let saved = Connection::open(&output).unwrap();
        let config: String = saved
            .query_row("SELECT config_json FROM sources", [], |row| row.get(0))
            .unwrap();
        assert!(config.contains("dpapi:encrypted-value"));
        drop(saved);
        drop(state);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn stage_rejects_broken_and_newer_schema_files() {
        let dir = temp_dir("invalid");
        let broken = dir.join("broken.db");
        fs::write(&broken, b"not sqlite").unwrap();
        assert!(stage_restore(&dir, &broken).is_err());

        let newer = dir.join("newer.db");
        create_database(&newer, "newer");
        let conn = Connection::open(&newer).unwrap();
        db::set_meta(&conn, "schema_version", "999").unwrap();
        drop(conn);
        assert!(stage_restore(&dir, &newer).is_err());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn pending_restore_replaces_database_and_keeps_rollback() {
        let dir = temp_dir("restore");
        let live = dir.join("cache.db");
        let incoming = dir.join("incoming.db");
        create_database(&live, "before");
        create_database(&incoming, "after");
        fs::write(dir.join("cache.db-wal"), b"old wal").unwrap();
        fs::write(dir.join("cache.db-shm"), b"old shm").unwrap();

        stage_restore(&dir, &incoming).unwrap();
        let rollback = apply_pending_restore(&dir).unwrap().unwrap();

        assert!(rollback.exists());
        assert!(!dir.join(PENDING_FILE).exists());
        if let Ok(contents) = fs::read(dir.join("cache.db-wal")) {
            assert_ne!(contents, b"old wal");
        }
        if let Ok(contents) = fs::read(dir.join("cache.db-shm")) {
            assert_ne!(contents, b"old shm");
        }
        let restored = Connection::open(&live).unwrap();
        let name: String = restored
            .query_row("SELECT name FROM sources", [], |row| row.get(0))
            .unwrap();
        assert_eq!(name, "after");
        drop(restored);
        fs::remove_dir_all(dir).unwrap();
    }
}

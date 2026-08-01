use anyhow::{anyhow, Context, Result};
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};

use super::{get_meta, set_meta};
pub(super) const SCHEMA_VERSION_KEY: &str = "schema_version";
const CURRENT_SCHEMA_VERSION: i64 = 2;

/// cache.db を開き、必要な場合だけ移行前バックアップを取ってから初期化する。
///
/// バックアップは移行成功時に削除する。移行に失敗した場合は元のDBファイルへ復旧してから
/// エラーを返すため、起動失敗によって既存データが中途半端な状態で残らない。
pub fn open_cached_database(path: &Path) -> Result<Connection> {
    initialize_database_with(path, init_schema)
}

pub(super) fn initialize_database_with<F>(path: &Path, initializer: F) -> Result<Connection>
where
    F: FnOnce(&Connection) -> Result<()>,
{
    let has_existing_data = path.is_file() && fs::metadata(path)?.len() > 0;
    let needs_migration = if has_existing_data {
        let conn = Connection::open(path)?;
        let needs_migration = schema_needs_migration(&conn)?;
        if needs_migration {
            // WAL に残った内容を本体へ反映してからコピーする。
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        }
        needs_migration
    } else {
        false
    };

    let backup = if needs_migration {
        Some(create_migration_backup(path)?)
    } else {
        None
    };

    let conn = Connection::open(path)?;
    if let Err(error) = initializer(&conn) {
        drop(conn);
        if let Some(backup_path) = backup.as_deref() {
            restore_migration_backup(path, backup_path)?;
            let _ = fs::remove_file(backup_path);
        }
        return Err(error)
            .context("DB schema migration failed; the previous database was restored");
    }

    if let Some(backup_path) = backup {
        fs::remove_file(&backup_path).with_context(|| {
            format!(
                "migration succeeded but temporary backup could not be removed: {}",
                backup_path.display()
            )
        })?;
    }
    Ok(conn)
}

fn create_migration_backup(path: &Path) -> Result<PathBuf> {
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow!("database path has no file name: {}", path.display()))?
        .to_string_lossy();
    let backup_path = path.with_file_name(format!("{file_name}.before-migration.bak"));
    if backup_path.exists() {
        return Err(anyhow!(
            "existing migration backup must be inspected before retrying: {}",
            backup_path.display()
        ));
    }
    fs::copy(path, &backup_path).with_context(|| {
        format!(
            "failed to create migration backup: {} -> {}",
            path.display(),
            backup_path.display()
        )
    })?;
    Ok(backup_path)
}

fn restore_migration_backup(path: &Path, backup_path: &Path) -> Result<()> {
    fs::copy(backup_path, path).with_context(|| {
        format!(
            "failed to restore migration backup: {} -> {}",
            backup_path.display(),
            path.display()
        )
    })?;
    Ok(())
}

/// スキーマ初期化。既存DBはschema_versionに応じて、列追加をトランザクション内で段階適用する。
pub fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;",
    )?;
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS sources (
                id               INTEGER PRIMARY KEY,
                type             TEXT NOT NULL,
                name             TEXT NOT NULL,
                config_json      TEXT NOT NULL,
                refresh_interval INTEGER NOT NULL DEFAULT 3600,
                status           TEXT,
                last_success_at  TEXT,
                last_error       TEXT,
                sort_order       INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS app_meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS search_history (
                id         INTEGER PRIMARY KEY,
                query      TEXT NOT NULL,
                used_at    TEXT NOT NULL DEFAULT (datetime('now','localtime'))
            );

            CREATE TABLE IF NOT EXISTS search_presets (
                id              INTEGER PRIMARY KEY,
                name            TEXT NOT NULL,
                query           TEXT NOT NULL DEFAULT '',
                source_ids_json TEXT NOT NULL DEFAULT '[]',
                created_at      TEXT NOT NULL DEFAULT (datetime('now','localtime'))
            );

            CREATE TABLE IF NOT EXISTS credentials (
                id          INTEGER PRIMARY KEY,
                scope       TEXT NOT NULL,
                label       TEXT NOT NULL,
                fields_json TEXT NOT NULL,
                created_at  TEXT NOT NULL DEFAULT (datetime('now','localtime'))
            );

            CREATE TABLE IF NOT EXISTS records (
                id          INTEGER PRIMARY KEY,
                source_id   INTEGER NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
                label       TEXT,
                open_ref    TEXT,
                row_number  INTEGER,
                fields_json TEXT NOT NULL,
                search_text TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_records_source ON records(source_id);

            CREATE VIRTUAL TABLE IF NOT EXISTS records_fts USING fts5(
                search_text,
                content='records',
                content_rowid='id',
                tokenize='trigram'
            );

            CREATE TRIGGER IF NOT EXISTS records_ai AFTER INSERT ON records BEGIN
                INSERT INTO records_fts(rowid, search_text) VALUES (new.id, new.search_text);
            END;
            CREATE TRIGGER IF NOT EXISTS records_ad AFTER DELETE ON records BEGIN
                INSERT INTO records_fts(records_fts, rowid, search_text)
                  VALUES ('delete', old.id, old.search_text);
            END;
            CREATE TRIGGER IF NOT EXISTS records_au AFTER UPDATE ON records BEGIN
                INSERT INTO records_fts(records_fts, rowid, search_text)
                  VALUES ('delete', old.id, old.search_text);
                INSERT INTO records_fts(rowid, search_text) VALUES (new.id, new.search_text);
            END;
            "#,
        )?;

        let version = schema_version(conn)?;
        if version > CURRENT_SCHEMA_VERSION {
            return Err(anyhow!(
                "database schema version {version} is newer than supported version {CURRENT_SCHEMA_VERSION}"
            ));
        }
        if version < 1 {
            migrate_add_column(conn, "sources", "sort_order", "INTEGER NOT NULL DEFAULT 0")?;
        }
        if version < 2 {
            migrate_add_column(conn, "records", "row_number", "INTEGER")?;
        }
        set_meta(
            conn,
            SCHEMA_VERSION_KEY,
            &CURRENT_SCHEMA_VERSION.to_string(),
        )?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            conn.execute_batch("COMMIT")?;
            Ok(())
        }
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

fn schema_version(conn: &Connection) -> Result<i64> {
    match get_meta(conn, SCHEMA_VERSION_KEY)? {
        Some(value) => value
            .parse::<i64>()
            .with_context(|| format!("invalid {SCHEMA_VERSION_KEY}: {value}")),
        None => Ok(0),
    }
}

fn schema_needs_migration(conn: &Connection) -> Result<bool> {
    if !table_exists(conn, "app_meta")? {
        return Ok(true);
    }
    Ok(schema_version(conn)? < CURRENT_SCHEMA_VERSION
        || !table_has_column(conn, "sources", "sort_order")?
        || !table_has_column(conn, "records", "row_number")?)
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table],
        |row| row.get(0),
    )?)
}

pub(super) fn table_has_column(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let exists = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|row| row.ok())
        .any(|name| name == column);
    Ok(exists)
}

fn migrate_add_column(
    conn: &Connection,
    table: &str,
    column: &str,
    declaration: &str,
) -> Result<()> {
    if !table_has_column(conn, table, column)? {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {declaration}"),
            [],
        )?;
    }
    Ok(())
}

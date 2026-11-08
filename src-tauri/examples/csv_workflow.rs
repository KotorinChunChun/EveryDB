//! CSV検証CLIのバックエンド。GUIと同じ業務処理を、専用DBに対して実行する。

use anyhow::{bail, ensure, Context, Result};
use everydb_lib::{app_state::AppState, browse_service, db, search_service, source_service};
use serde::Serialize;
use std::{env, fs, io, path::Path};

fn output(value: impl Serialize) -> Result<()> {
    let stdout = io::stdout().lock();
    serde_json::to_writer(stdout, &value)?;
    println!();
    Ok(())
}

fn csv_config(csv: &str) -> Result<String> {
    let path = fs::canonicalize(csv).context("入力CSVを開けません")?;
    ensure!(path.is_file(), "入力CSVに通常のファイルを指定してください");
    // Windows の拡張パス表記はそのままファイルAPIへ渡す。
    Ok(serde_json::json!({ "path": path }).to_string())
}

fn isolated_state(directory: &str, create: bool) -> Result<AppState> {
    let directory = Path::new(directory);
    ensure!(
        directory.is_absolute(),
        "隔離フォルダーは絶対パスで指定してください"
    );
    let directory = fs::canonicalize(directory).context("隔離フォルダーがありません")?;
    ensure!(
        directory.is_dir()
            && directory
                .join("cache.db")
                .symlink_metadata()
                .is_err_and(|e| e.kind() == io::ErrorKind::NotFound),
        "通常のcache.dbを含まない隔離フォルダーを指定してください"
    );
    let path = directory.join("csv-cli.sqlite3");
    match path.symlink_metadata() {
        Ok(metadata) => ensure!(
            metadata.file_type().is_file(),
            "隔離DBは通常のファイルである必要があります"
        ),
        Err(error) if create && error.kind() == io::ErrorKind::NotFound => {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
        }
        Err(error) => return Err(error).context("隔離DBを開けません。先にCSVを取り込んでください"),
    }
    Ok(AppState::new(db::open_cached_database(&path)?))
}

fn run() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [command, csv] if command == "preview" => output(
            browse_service::preview_csv(&csv_config(csv)?).map_err(|e| anyhow::anyhow!("{e}"))?,
        ),
        [command, directory, csv, name] if command == "import" => {
            ensure!(!name.trim().is_empty(), "ソース名は必須です");
            let config = csv_config(csv)?;
            let state = isolated_state(directory, true)?;
            let id = source_service::add_source(&state, "csv".into(), name.clone(), config)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let source = source_service::list_sources(&state)
                .map_err(|e| anyhow::anyhow!("{e}"))?
                .into_iter()
                .find(|source| source.id == id)
                .context("追加したソースを確認できません")?;
            ensure!(
                source.status.as_deref() == Some("ok"),
                "CSVの初回取込に失敗しました（ソースID: {id}）"
            );
            output(serde_json::json!({"id": id, "record_count": source.record_count}))
        }
        [command, directory, query, ids @ ..] if command == "search" && !ids.is_empty() => {
            let ids = ids
                .iter()
                .map(|id| {
                    id.parse::<i64>()
                        .context("ソースIDは整数で指定してください")
                })
                .collect::<Result<Vec<_>>>()?;
            let state = isolated_state(directory, false)?;
            let sources =
                source_service::list_sources(&state).map_err(|e| anyhow::anyhow!("{e}"))?;
            ensure!(
                ids.iter()
                    .all(|id| sources.iter().any(|source| source.id == *id
                        && source.type_ == "csv"
                        && source.status.as_deref() == Some("ok"))),
                "取込済みCSVのソースIDを指定してください"
            );
            // 検索履歴は明示的に保存しない。
            output(
                search_service::search(&state, query, &ids, 0, None)
                    .map_err(|e| anyhow::anyhow!("{e}"))?,
            )
        }
        _ => bail!("引数が不正です。node scripts/everydb-csv-cli.mjs --help を参照してください"),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

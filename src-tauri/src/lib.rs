pub mod app_state;
pub mod backup_commands;
pub mod backup_service;
pub mod box_oauth;
pub mod browse;
pub mod browse_commands;
pub mod browse_service;
pub mod connectors;
pub mod credential_commands;
#[cfg(test)]
mod credential_regression;
pub mod credential_service;
pub mod credentials;
pub mod db;
pub mod export_commands;
pub mod export_service;
pub mod graph;
pub mod graph_oauth;
#[cfg(test)]
mod input_boundary_audit;
pub mod internal_log;
pub mod lifecycle;
pub mod management;
pub mod model;
pub mod opener;
#[cfg(test)]
mod perf_baselines;
pub mod refresh;
pub mod refresh_service;
pub mod search_commands;
pub mod search_service;
pub mod secret;
#[cfg(test)]
mod security_audit;
pub mod service_error;
pub mod settings;
pub mod settings_service;
pub mod source_commands;
pub mod source_service;
#[cfg(test)]
mod test_fixtures;
pub mod winauth;

use std::path::PathBuf;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, RunEvent, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

mod window_title;

use app_state::AppState;
use lifecycle::{cancel_oauth_tasks, shutdown_background_tasks};

/// トレイの「表示」・グローバルホットキーで共通のウィンドウ表示処理。
/// 表示後に検索ボックスへフォーカスさせるイベントを送る（要件 §9-5: Everything ライク挙動）。
fn show_main_window(app: &tauri::AppHandle) -> bool {
    if let Some(w) = app.get_webview_window("main") {
        let visible = match w.show() {
            Ok(()) => true,
            Err(error) => {
                internal_log::warn("window", format!("show failed: {error}"));
                false
            }
        };
        let focused = match w.set_focus() {
            Ok(()) => true,
            Err(error) => {
                internal_log::warn("window", format!("focus failed: {error}"));
                false
            }
        };
        if let Err(error) = app.emit("focus-search", ()) {
            internal_log::warn("window", format!("focus-search emit failed: {error}"));
        }
        return visible && focused;
    }
    false
}

fn app_data_dir(app: &tauri::AppHandle) -> anyhow::Result<PathBuf> {
    #[cfg(debug_assertions)]
    if let Some(directory) = std::env::var_os("EVERYDB_TEST_DATA_DIR") {
        let directory = PathBuf::from(directory);
        anyhow::ensure!(
            directory.is_absolute(),
            "Debug試験用データ保存先は絶対パスで指定してください"
        );
        return Ok(directory);
    }
    Ok(app.path().app_data_dir()?)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        // Windows ログイン時の自動起動時は "--minimized" 付きで起動させ、
        // setup 側でそれを検知してウィンドウを出さずトレイ常駐のまま待機する。
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        show_main_window(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            // キャッシュ DB を初期化して状態管理下に置く。
            let dir = app_data_dir(app.handle())?;
            std::fs::create_dir_all(&dir)?;
            backup_service::apply_pending_restore(&dir)?;
            let conn = db::open_cached_database(&dir.join("cache.db"))?;
            // 前回のズーム倍率とホットキーを復元する（要件 REQ-UI-01 / §9-5）。
            let saved_zoom: f64 = db::get_meta(&conn, "ui_zoom")?
                .and_then(|s| s.parse().ok())
                .unwrap_or(1.0);
            let saved_hotkey: String =
                db::get_meta(&conn, "ui_hotkey")?.unwrap_or_else(|| "Alt+Ctrl+D".to_string());
            app.manage(AppState::new(conn));
            let scheduler_shutdown = {
                let state: tauri::State<AppState> = app.state();
                state.scheduler_shutdown.clone()
            };
            if let Some(w) = app.get_webview_window("main") {
                if let Err(error) = w.set_title(&window_title::WindowTitle::create()) {
                    internal_log::warn("window", format!("title update failed: {error}"));
                }
                if let Err(error) = w.set_zoom(saved_zoom) {
                    internal_log::warn("window", format!("restore zoom failed: {error}"));
                }
            }

            // トレイ常駐（表示 / 全ソース更新 / 終了）。
            let show = MenuItem::with_id(app, "show", "表示", true, None::<&str>)?;
            let reload = MenuItem::with_id(app, "reload", "全ソースを更新", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "終了", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &reload, &quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("EveryDB")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        let _ = show_main_window(app);
                    }
                    "reload" => refresh::reload_all_now(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // グローバルホットキーでウィンドウ表示（要件 §9-5 / Everything ライク）。
            if let Err(error) = app.global_shortcut().register(saved_hotkey.as_str()) {
                internal_log::warn(
                    "hotkey",
                    format!("startup register failed for '{saved_hotkey}': {error}"),
                );
            }

            // 定期自動更新タイマーを起動。
            refresh::spawn_scheduler(app.handle().clone(), scheduler_shutdown);
            app.manage(management::Management::start(app.handle().clone())?);
            Ok(())
        })
        // ウィンドウの×は終了せずトレイに格納する。OAuth 実行中なら取消する。
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    cancel_oauth_tasks(window.app_handle());
                    if let Err(error) = window.hide() {
                        internal_log::warn("window", format!("hide failed: {error}"));
                    }
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            source_commands::list_sources,
            source_commands::add_source,
            source_commands::delete_source,
            source_commands::set_refresh_interval,
            source_commands::reload_sources,
            backup_commands::export_database_backup,
            backup_commands::stage_database_restore,
            search_commands::search,
            search_commands::add_search_history_entry,
            search_commands::list_search_history,
            search_commands::save_search_preset,
            search_commands::list_search_presets,
            search_commands::delete_search_preset,
            opener::open_ref,
            export_commands::encode_shift_jis,
            credential_commands::winauth_status,
            credential_commands::winauth_verify,
            source_commands::get_source_detail,
            source_commands::update_source,
            source_commands::duplicate_source,
            source_commands::reorder_sources,
            source_commands::preview_source_fields,
            source_commands::preview_source_fields_by_id,
            source_commands::add_source_from_sibling,
            browse_commands::browse_notion_databases,
            browse_commands::browse_notion_databases_for_source,
            browse_commands::browse_notion_database,
            browse_commands::browse_notion_database_for_source,
            browse_commands::preview_csv,
            browse_commands::browse_airtable_bases,
            browse_commands::browse_airtable_bases_for_source,
            browse_commands::browse_airtable_tables,
            browse_commands::browse_airtable_tables_for_source,
            browse_commands::browse_graph_site_resources_for_source,
            browse_commands::browse_graph_sites,
            browse_commands::browse_graph_teams,
            browse_commands::browse_graph_team_site_url,
            browse_commands::browse_graph_site_resources,
            browse_commands::browse_excel_sheets,
            browse_commands::browse_excel_tables,
            browse_commands::browse_excel_grid,
            browse_commands::browse_sqlite_tables,
            browse_commands::browse_accdb_tables,
            browse_commands::browse_browser_profiles,
            credential_commands::list_credentials,
            credential_commands::add_credential,
            credential_commands::update_credential,
            credential_commands::delete_credential,
            credential_commands::credential_usage,
            credential_commands::credential_public_fields,
            credential_commands::credential_permissions,
            credential_commands::credential_secret_value,
            credential_commands::box_oauth_authorize,
            credential_commands::box_oauth_cancel,
            credential_commands::graph_oauth_authorize,
            credential_commands::microsoft_account_oauth_authorize,
            credential_commands::graph_oauth_cancel,
            settings::autostart_status,
            settings::autostart_set,
            settings::set_zoom,
            settings::get_zoom,
            settings::set_theme,
            settings::get_theme,
            settings::set_hotkey,
            settings::get_hotkey,
            settings::set_sidebar_width,
            settings::get_sidebar_width,
            settings::list_parent_labels,
            settings::set_parent_label,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            RunEvent::Ready => {
                // Tauriが初期ウィンドウを表示した後で隠し、管理側のReady応答前に反映する。
                let startup =
                    std::env::args().any(|arg| arg == "--minimized" || arg == "--startup");
                if startup {
                    if let Some(window) = app.get_webview_window("main") {
                        if let Err(error) = window.hide() {
                            internal_log::warn("window", format!("minimized hide failed: {error}"));
                        }
                    }
                } else {
                    let _ = show_main_window(app);
                }
                if let Some(management) = app.try_state::<management::Management>() {
                    management.set_ready();
                }
            }
            RunEvent::Exit => shutdown_background_tasks(app),
            _ => {}
        });
}

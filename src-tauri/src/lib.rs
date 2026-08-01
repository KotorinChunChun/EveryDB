pub mod app_state;
pub mod box_oauth;
pub mod browse;
pub mod browse_commands;
mod commands;
pub mod connectors;
pub mod credential_commands;
pub mod credentials;
pub mod db;
pub mod graph;
pub mod model;
pub mod opener;
pub mod refresh;
pub mod search_commands;
pub mod secret;
pub mod settings;
pub mod winauth;

use rusqlite::Connection;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use app_state::AppState;

/// トレイの「表示」・グローバルホットキーで共通のウィンドウ表示処理。
/// 表示後に検索ボックスへフォーカスさせるイベントを送る（要件 §9-5: Everything ライク挙動）。
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
        let _ = app.emit("focus-search", ());
    }
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
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = Connection::open(dir.join("cache.db"))?;
            db::init_schema(&conn)?;
            // 前回のズーム倍率とホットキーを復元する（要件 REQ-UI-01 / §9-5）。
            let saved_zoom: f64 = db::get_meta(&conn, "ui_zoom")?
                .and_then(|s| s.parse().ok())
                .unwrap_or(1.0);
            let saved_hotkey: String =
                db::get_meta(&conn, "ui_hotkey")?.unwrap_or_else(|| "Alt+Ctrl+D".to_string());
            app.manage(AppState::new(conn));
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_zoom(saved_zoom);
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
                    "show" => show_main_window(app),
                    "reload" => refresh::reload_all_now(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;

            // グローバルホットキーでウィンドウ表示（要件 §9-5 / Everything ライク）。
            let _ = app.global_shortcut().register(saved_hotkey.as_str());

            // "--minimized"（自動起動経由）の場合はウィンドウを出さずトレイ常駐のまま待機する。
            let launched_minimized = std::env::args().any(|a| a == "--minimized");
            if launched_minimized {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }

            // 定期自動更新タイマーを起動。
            refresh::spawn_scheduler(app.handle().clone());
            Ok(())
        })
        // ウィンドウの×は終了せずトレイに格納する。
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_sources,
            commands::add_source,
            commands::delete_source,
            commands::set_refresh_interval,
            commands::reload_sources,
            search_commands::search,
            search_commands::add_search_history_entry,
            search_commands::list_search_history,
            search_commands::save_search_preset,
            search_commands::list_search_presets,
            search_commands::delete_search_preset,
            opener::open_ref,
            commands::encode_shift_jis,
            credential_commands::winauth_status,
            credential_commands::winauth_verify,
            commands::get_source_detail,
            commands::update_source,
            commands::duplicate_source,
            commands::reorder_sources,
            commands::preview_source_fields,
            commands::preview_source_fields_by_id,
            commands::add_source_from_sibling,
            browse_commands::browse_notion_databases,
            browse_commands::browse_notion_databases_for_source,
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
            credential_commands::credential_secret_value,
            credential_commands::box_oauth_authorize,
            credential_commands::box_oauth_cancel,
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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

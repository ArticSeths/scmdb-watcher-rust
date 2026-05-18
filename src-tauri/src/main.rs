#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Listener, Manager, WindowEvent,
};
use tokio::sync::Mutex;
use tracing::info;
use tracing_subscriber::EnvFilter;

use scmdb_watcher::commands::{
    self, AppState,
};
use scmdb_watcher::config::AppConfig;
use scmdb_watcher::watcher::bus::EventBus;
use scmdb_watcher::watcher::state::WatcherState;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = AppConfig::load();
    let auto_start = config.auto_start_watcher;

    let app_state = AppState {
        config: Arc::new(Mutex::new(config)),
        watcher_state: WatcherState::new(),
        event_bus: EventBus::new(),
        stop_tx: Arc::new(Mutex::new(None)),
        running: Arc::new(Mutex::new(false)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::save_config,
            commands::start_watcher,
            commands::stop_watcher,
            commands::get_watcher_status,
            commands::get_active_missions,
            commands::run_import_command,
            commands::export_import_json,
        ])
        .setup(move |app| {
            let window = app.get_webview_window("main").unwrap();

            // --- System Tray ---
            let status_item = MenuItemBuilder::with_id("status", "● Stopped")
                .enabled(false)
                .build(app)?;
            let toggle_item = MenuItemBuilder::with_id("toggle", "Start Watcher").build(app)?;
            let show_item = MenuItemBuilder::with_id("show", "Show Window").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&status_item)
                .separator()
                .item(&toggle_item)
                .item(&show_item)
                .separator()
                .item(&quit_item)
                .build()?;

            let tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().cloned().unwrap())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("SCMDB Watcher — Stopped")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "toggle" => {
                        let app_handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let state = app_handle.state::<AppState>();
                            let running = *state.running.lock().await;
                            if running {
                                let _ = commands::stop_watcher(app_handle.clone(), state).await;
                            } else {
                                let _ = commands::start_watcher(app_handle.clone(), state).await;
                            }
                        });
                    }
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.unminimize();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => {
                        let app_handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            let state = app_handle.state::<AppState>();
                            let running = *state.running.lock().await;
                            if running {
                                let _ = commands::stop_watcher(app_handle.clone(), state).await;
                            }
                            app_handle.exit(0);
                        });
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    match event {
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } => {
                            let app = tray.app_handle();
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.unminimize();
                                let _ = w.set_focus();
                            }
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            // --- Listen for status changes to update tray ---
            let tray_handle = tray.clone();
            let status_item_clone = status_item.clone();
            let toggle_item_clone = toggle_item.clone();
            let handle = app.handle().clone();
            handle.listen("watcher-status-change", move |event| {
                let payload = event.payload();
                let is_running = payload.contains("running");
                let _ = status_item_clone.set_text(if is_running {
                    "● Running"
                } else {
                    "● Stopped"
                });
                let _ = toggle_item_clone.set_text(if is_running {
                    "Stop Watcher"
                } else {
                    "Start Watcher"
                });
                let _ = tray_handle.set_tooltip(Some(if is_running {
                    "SCMDB Watcher — Running"
                } else {
                    "SCMDB Watcher — Stopped"
                }));
            });

            // --- Hide to tray on minimize ---
            let win = window.clone();
            window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = win.hide();
                }
            });

            // --- Auto-start watcher if configured ---
            if auto_start {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    // Small delay to let frontend connect
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    let state = app_handle.state::<AppState>();
                    if let Err(e) = commands::start_watcher(app_handle.clone(), state).await {
                        tracing::warn!("Auto-start failed: {}", e);
                    }
                });
            }

            info!("SCMDB Watcher v{} initialized", env!("CARGO_PKG_VERSION"));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Error running SCMDB Watcher");
}

use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::{watch, Mutex};
use tracing::{error, info};

use crate::config::AppConfig;
use crate::importer;
use crate::server::start_sse_server;
use crate::watcher::bus::EventBus;
use crate::watcher::state::WatcherState;
use crate::watcher::tailer::LogTailer;

pub struct AppState {
    pub config: Arc<Mutex<AppConfig>>,
    pub watcher_state: WatcherState,
    pub event_bus: EventBus,
    pub stop_tx: Arc<Mutex<Option<watch::Sender<bool>>>>,
    pub running: Arc<Mutex<bool>>,
}

#[tauri::command]
pub fn is_dev_build() -> bool {
    cfg!(debug_assertions)
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let config = state.config.lock().await;
    Ok(config.clone())
}

#[tauri::command]
pub async fn save_config(state: State<'_, AppState>, config: AppConfig) -> Result<(), String> {
    config.save()?;
    let mut current = state.config.lock().await;
    *current = config;
    Ok(())
}

#[tauri::command]
pub async fn start_watcher(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let mut running = state.running.lock().await;
    if *running {
        return Err("Watcher is already running".to_string());
    }

    let config = state.config.lock().await.clone();
    let watcher_state = state.watcher_state.clone();
    let event_bus = state.event_bus.clone();

    let (stop_tx, stop_rx) = watch::channel(false);
    *state.stop_tx.lock().await = Some(stop_tx);

    let log_path = PathBuf::from(&config.log_path);
    let port = config.port;
    let allowed_origins = config.allowed_origins();

    // Start tailer task
    let tailer = LogTailer::new(log_path, watcher_state.clone(), event_bus.clone(), stop_rx);
    tokio::spawn(async move {
        tailer.run().await;
    });

    // Start SSE server task
    let ws = watcher_state.clone();
    let eb = event_bus.clone();
    tokio::spawn(async move {
        if let Err(e) = start_sse_server(port, allowed_origins, ws, eb).await {
            error!("SSE server error: {}", e);
        }
    });

    // Forward events to frontend
    let mut rx = event_bus.subscribe();
    let app_handle = app.clone();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let _ = app_handle.emit("watcher-event", &event);
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    });

    *running = true;
    let _ = app.emit("watcher-status-change", "running");
    info!("Watcher started on port {}", port);
    Ok(())
}

#[tauri::command]
pub async fn stop_watcher(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let mut running = state.running.lock().await;
    if !*running {
        return Err("Watcher is not running".to_string());
    }

    if let Some(tx) = state.stop_tx.lock().await.take() {
        let _ = tx.send(true);
    }

    *running = false;
    let _ = app.emit("watcher-status-change", "stopped");
    info!("Watcher stopped");
    Ok(())
}

#[tauri::command]
pub async fn get_watcher_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let running = *state.running.lock().await;
    let active = state.watcher_state.inner.lock().await.snapshot_active();
    let config = state.config.lock().await;
    Ok(serde_json::json!({
        "running": running,
        "port": config.port,
        "logPath": config.log_path,
        "activeMissions": active,
    }))
}

#[tauri::command]
pub async fn get_active_missions(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let active = state.watcher_state.inner.lock().await.snapshot_active();
    Ok(serde_json::json!({ "active": active }))
}

#[tauri::command]
pub async fn run_import_command(
    logbackups_dir: String,
    include_current: bool,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let config = state.config.lock().await;
    let log_path = PathBuf::from(&config.log_path);
    let backups_dir = PathBuf::from(&logbackups_dir);

    let result = tokio::task::spawn_blocking(move || {
        importer::run_import(&backups_dir, include_current, Some(&log_path))
    })
    .await
    .map_err(|e| e.to_string())??;

    serde_json::to_value(result).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_import_json(
    data: serde_json::Value,
    output_path: String,
) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    std::fs::write(&output_path, json).map_err(|e| e.to_string())
}

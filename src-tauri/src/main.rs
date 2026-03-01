#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod idclass;
mod collector;
mod state;
mod sync;

use log::info;
use serde::Serialize;
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
    State as TauriState,
};
use tokio::sync::Mutex;

#[derive(Clone)]
struct AppRuntimeState {
    sync_lock: Arc<Mutex<()>>,
}

#[derive(Debug, Serialize)]
struct SyncStatus {
    last_synced_at: Option<String>,
    last_nsr: u64,
    last_records_sent: u32,
    last_message: String,
    sync_interval_secs: u64,
    next_sync_at: Option<String>,
}

async fn run_sync_once(lock: Arc<Mutex<()>>) -> Result<sync::SyncResult, String> {
    let _guard = lock.lock().await;
    let config = config::load_config().map_err(|e| e.to_string())?;
    sync::sync(&config).await
}

fn get_sync_status_data() -> Result<SyncStatus, String> {
    let current_state = state::load_state().map_err(|e| e.to_string())?;
    let logs = state::load_logs().map_err(|e| e.to_string())?;
    let config = config::load_config().map_err(|e| e.to_string())?;

    let last_synced_at = if current_state.last_synced_at == chrono::DateTime::<chrono::Utc>::MIN_UTC {
        None
    } else {
        Some(current_state.last_synced_at.to_rfc3339())
    };

    let (last_records_sent, last_message) = logs
        .entries
        .iter()
        .find(|entry| entry.status != "info")
        .map(|entry| (entry.records_sent, entry.message.clone()))
        .unwrap_or((0, String::new()));

    let next_sync_at = last_synced_at.as_ref().and_then(|value| {
        chrono::DateTime::parse_from_rfc3339(value)
            .ok()
            .map(|dt| (dt + chrono::Duration::seconds(config.sync_interval_secs as i64)).to_utc().to_rfc3339())
    });

    Ok(SyncStatus {
        last_synced_at,
        last_nsr: current_state.last_nsr,
        last_records_sent,
        last_message,
        sync_interval_secs: config.sync_interval_secs,
        next_sync_at,
    })
}

#[tauri::command]
fn save_config(
    device_ip: String,
    device_user: String,
    device_password: String,
    api_key: String,
    clock_id: String,
    sync_interval_secs: u64,
) -> Result<(), String> {
    config::save_config(&config::Config {
        device_ip,
        device_user,
        device_password,
        api_key,
        clock_id,
        sync_interval_secs,
    }).map_err(|e| e.to_string())
}

#[tauri::command]
fn load_config() -> Result<config::Config, String> {
    config::load_config().map_err(|e| e.to_string())
}

#[tauri::command]
async fn test_connection(device_ip: String, device_user: String, device_password: String) -> Result<bool, String> {
    let result = idclass::login(&device_ip, &device_user, &device_password).await;
    match result {
        Ok(_) => Ok(true),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
async fn sync_now() -> Result<sync::SyncResult, String> {
    let lock = Arc::new(Mutex::new(()));
    run_sync_once(lock).await
}

#[tauri::command]
async fn sync_now_locked(app_state: TauriState<'_, AppRuntimeState>) -> Result<sync::SyncResult, String> {
    run_sync_once(app_state.sync_lock.clone()).await
}

#[tauri::command]
fn get_sync_status() -> Result<SyncStatus, String> {
    get_sync_status_data()
}

#[tauri::command]
fn get_logs() -> Result<Vec<state::LogEntry>, String> {
    let logs = state::load_logs().map_err(|e| e.to_string())?;
    Ok(logs.entries)
}

#[tauri::command]
async fn reset_sync_state(app_state: TauriState<'_, AppRuntimeState>) -> Result<(), String> {
    let _guard = app_state.sync_lock.lock().await;

    state::save_state(&state::State::default()).map_err(|e| e.to_string())?;
    let _ = state::save_log("success", 0, "Cursor de sincronização resetado manualmente (NSR=0)");

    Ok(())
}

#[tauri::command]
async fn reprocess_history_locked(app_state: TauriState<'_, AppRuntimeState>) -> Result<sync::SyncResult, String> {
    let _guard = app_state.sync_lock.lock().await;

    state::save_state(&state::State::default()).map_err(|e| e.to_string())?;
    let _ = state::save_log("success", 0, "Cursor de sincronização resetado manualmente (NSR=0)");

    let config = config::load_config().map_err(|e| e.to_string())?;
    sync::sync(&config).await
}

fn main() {
    env_logger::init();
    info!("Starting Ryanne Ponto Agent");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppRuntimeState {
            sync_lock: Arc::new(Mutex::new(())),
        })
        .setup(|app| {
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    let sync_lock = {
                        let state = app_handle.state::<AppRuntimeState>();
                        state.sync_lock.clone()
                    };

                    let config = match config::load_config() {
                        Ok(value) => value,
                        Err(error) => {
                            log::error!("Failed to load config for scheduler: {}", error);
                            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                            continue;
                        }
                    };

                    if !config.device_ip.is_empty()
                        && !config.device_password.is_empty()
                        && !config.api_key.is_empty()
                    {
                        if let Err(error) = run_sync_once(sync_lock).await {
                            log::error!("Scheduled sync failed: {}", error);
                        }
                    }

                    tokio::time::sleep(tokio::time::Duration::from_secs(config.sync_interval_secs.max(60))).await;
                }
            });

            let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
            let show = MenuItem::with_id(app, "show", "Abrir configurações", true, None::<&str>)?;
            let sync = MenuItem::with_id(app, "sync", "Sincronizar agora", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &sync, &quit])?;

            let _tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("Ryanne Ponto Agent")
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "quit" => {
                            info!("Quit requested from tray");
                            app.exit(0);
                        }
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "sync" => {
                            info!("Manual sync requested from tray");
                            let app_handle = app.clone();
                            tauri::async_runtime::spawn(async move {
                                let sync_lock = {
                                    let state = app_handle.state::<AppRuntimeState>();
                                    state.sync_lock.clone()
                                };

                                if let Err(error) = run_sync_once(sync_lock).await {
                                    log::error!("Manual sync from tray failed: {}", error);
                                }
                            });
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            save_config,
            load_config,
            test_connection,
            sync_now,
            sync_now_locked,
            get_sync_status,
            get_logs,
            reset_sync_state,
            reprocess_history_locked,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod idclass;
mod collector;
mod state;
mod sync;

use log::{info, error};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

#[tauri::command]
fn save_config(
    device_ip: String,
    device_user: String,
    device_password: String,
    app_url: String,
    api_key: String,
    sync_interval_secs: u64,
) -> Result<(), String> {
    config::save_config(&config::Config {
        device_ip,
        device_user,
        device_password,
        app_url,
        api_key,
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
    let config = config::load_config().map_err(|e| e.to_string())?;
    sync::sync(&config).await.map_err(|e| e.to_string())
}

#[tauri::command]
fn get_logs() -> Result<Vec<state::LogEntry>, String> {
    let logs = state::load_logs().map_err(|e| e.to_string())?;
    Ok(logs.entries)
}

fn main() {
    env_logger::init();
    info!("Starting Ryanne Ponto Agent");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
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
            get_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

mod caddy;
mod commands;
mod config;
mod docker;
mod models;
mod watcher;

use std::sync::Arc;

use bollard::Docker;
use reqwest::Client;
use tokio::sync::Mutex;

use crate::models::Gateway;

pub struct AppState {
    pub docker: Docker,
    pub http_client: Client,
    pub auto_gateways: Arc<Mutex<Vec<Gateway>>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let docker = docker::connect().expect("Failed to connect to Docker");
    let http_client = Client::new();
    let auto_gateways = Arc::new(Mutex::new(Vec::new()));

    let state = AppState {
        docker: docker.clone(),
        http_client: http_client.clone(),
        auto_gateways: auto_gateways.clone(),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_caddy_status,
            commands::start_caddy,
            commands::get_routes,
            commands::get_all_gateways,
            commands::add_route,
            commands::remove_route,
            commands::get_settings,
            commands::save_settings,
            commands::get_route_rules,
            commands::add_route_rule,
            commands::remove_route_rule,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(watcher::run(
                docker,
                http_client,
                auto_gateways,
                handle,
            ));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

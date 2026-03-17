mod caddy;
mod commands;
mod config;
mod docker;
mod models;

use bollard::Docker;
use reqwest::Client;
use tokio::sync::Mutex;

pub struct AppStateInner {
    pub docker: Docker,
    pub http_client: Client,
}

pub struct AppState {
    pub inner: Mutex<AppStateInner>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let docker = docker::connect().expect("Failed to connect to Docker");
    let http_client = Client::new();

    let state = AppState {
        inner: Mutex::new(AppStateInner {
            docker,
            http_client,
        }),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_caddy_status,
            commands::start_caddy,
            commands::get_routes,
            commands::add_route,
            commands::remove_route,
            commands::get_settings,
            commands::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

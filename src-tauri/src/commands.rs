use tauri::State;

use crate::caddy;
use crate::config;
use crate::docker;
use crate::models::{AppConfig, CaddyStatus, Gateway};
use crate::AppState;

#[tauri::command]
pub async fn get_caddy_status(state: State<'_, AppState>) -> Result<CaddyStatus, String> {
    let app = state.inner.lock().await;
    let container_running = docker::is_caddy_running(&app.docker).await;
    let api_reachable = caddy::check_health(&app.http_client).await;
    Ok(CaddyStatus {
        running: container_running,
        api_reachable,
        error: None,
    })
}

#[tauri::command]
pub async fn start_caddy(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<CaddyStatus, String> {
    let app = state.inner.lock().await;
    let config = config::load_config(&app_handle);

    // Ensure network and container
    if let Err(e) = docker::ensure_network(&app.docker).await {
        return Ok(CaddyStatus {
            running: false,
            api_reachable: false,
            error: Some(e),
        });
    }

    if let Err(e) = docker::ensure_caddy(&app.docker, &config.caddy_image).await {
        return Ok(CaddyStatus {
            running: false,
            api_reachable: false,
            error: Some(e),
        });
    }

    // Wait briefly for Caddy API to become available
    let mut api_ready = false;
    for _ in 0..10 {
        if caddy::check_health(&app.http_client).await {
            api_ready = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    // Push stored routes if API is ready
    if api_ready && !config.static_routes.is_empty() {
        let _ = caddy::push_routes(&app.http_client, &config.static_routes, &config.domain).await;
    }

    Ok(CaddyStatus {
        running: true,
        api_reachable: api_ready,
        error: if api_ready {
            None
        } else {
            Some("Caddy started but API not yet reachable".to_string())
        },
    })
}

#[tauri::command]
pub async fn get_routes(app_handle: tauri::AppHandle) -> Result<Vec<Gateway>, String> {
    let config = config::load_config(&app_handle);
    Ok(config.static_routes)
}

#[tauri::command]
pub async fn add_route(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    subdomain: String,
    target_host: String,
    port: u16,
) -> Result<Vec<Gateway>, String> {
    let mut app_config = config::load_config(&app_handle);

    // Check for duplicate subdomain
    if app_config.static_routes.iter().any(|r| r.subdomain == subdomain) {
        return Err(format!("Route for subdomain '{subdomain}' already exists"));
    }

    app_config.static_routes.push(Gateway {
        subdomain,
        target_host,
        port,
    });

    config::save_config(&app_handle, &app_config)?;

    // Push to Caddy
    let app = state.inner.lock().await;
    let _ = caddy::push_routes(&app.http_client, &app_config.static_routes, &app_config.domain).await;

    Ok(app_config.static_routes)
}

#[tauri::command]
pub async fn remove_route(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    subdomain: String,
) -> Result<Vec<Gateway>, String> {
    let mut app_config = config::load_config(&app_handle);
    app_config.static_routes.retain(|r| r.subdomain != subdomain);
    config::save_config(&app_handle, &app_config)?;

    // Push to Caddy
    let app = state.inner.lock().await;
    let _ = caddy::push_routes(&app.http_client, &app_config.static_routes, &app_config.domain).await;

    Ok(app_config.static_routes)
}

#[tauri::command]
pub async fn get_settings(app_handle: tauri::AppHandle) -> Result<AppConfig, String> {
    Ok(config::load_config(&app_handle))
}

#[tauri::command]
pub async fn save_settings(
    app_handle: tauri::AppHandle,
    domain: String,
    caddy_image: String,
) -> Result<AppConfig, String> {
    let mut app_config = config::load_config(&app_handle);
    app_config.domain = domain;
    app_config.caddy_image = caddy_image;
    config::save_config(&app_handle, &app_config)?;
    Ok(app_config)
}

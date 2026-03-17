use tauri::{Emitter, State};

use crate::caddy;
use crate::config;
use crate::docker;
use crate::models::{AppConfig, CaddyStatus, Gateway, StaticRouteRule};
use crate::watcher;
use crate::AppState;

#[tauri::command]
pub async fn get_caddy_status(state: State<'_, AppState>) -> Result<CaddyStatus, String> {
    let container_running = docker::is_caddy_running(&state.docker).await;
    let api_reachable = caddy::check_health(&state.http_client).await;
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
    let config = config::load_config(&app_handle);

    if let Err(e) = docker::ensure_network(&state.docker).await {
        return Ok(CaddyStatus {
            running: false,
            api_reachable: false,
            error: Some(e),
        });
    }

    if let Err(e) = docker::ensure_caddy(&state.docker, &config.caddy_image).await {
        return Ok(CaddyStatus {
            running: false,
            api_reachable: false,
            error: Some(e),
        });
    }

    // Wait for Caddy API
    let mut api_ready = false;
    for _ in 0..10 {
        if caddy::check_health(&state.http_client).await {
            api_ready = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    // Push all routes (static + auto)
    if api_ready {
        let auto = state.auto_gateways.lock().await;
        let combined = watcher::combine_routes(&config.static_routes, &auto);
        let _ = caddy::push_routes(&state.http_client, &combined, &config.domain).await;
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
pub async fn get_all_gateways(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<Vec<Gateway>, String> {
    let config = config::load_config(&app_handle);
    let auto = state.auto_gateways.lock().await;
    Ok(watcher::combine_routes(&config.static_routes, &auto))
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

    if app_config
        .static_routes
        .iter()
        .any(|r| r.subdomain == subdomain)
    {
        return Err(format!("Route for subdomain '{subdomain}' already exists"));
    }

    app_config.static_routes.push(Gateway {
        subdomain,
        target_host,
        port,
        source: crate::models::GatewaySource::Static,
        container_id: None,
        container_name: None,
    });

    config::save_config(&app_handle, &app_config)?;

    // Push combined routes to Caddy
    let auto = state.auto_gateways.lock().await;
    let combined = watcher::combine_routes(&app_config.static_routes, &auto);
    let _ = caddy::push_routes(&state.http_client, &combined, &app_config.domain).await;

    // Emit update
    let _ = app_handle.emit("gateways-changed", &combined);

    Ok(app_config.static_routes)
}

#[tauri::command]
pub async fn remove_route(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    subdomain: String,
) -> Result<Vec<Gateway>, String> {
    let mut app_config = config::load_config(&app_handle);
    app_config
        .static_routes
        .retain(|r| r.subdomain != subdomain);
    config::save_config(&app_handle, &app_config)?;

    // Push combined routes
    let auto = state.auto_gateways.lock().await;
    let combined = watcher::combine_routes(&app_config.static_routes, &auto);
    let _ = caddy::push_routes(&state.http_client, &combined, &app_config.domain).await;

    let _ = app_handle.emit("gateways-changed", &combined);

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

#[tauri::command]
pub async fn get_route_rules(app_handle: tauri::AppHandle) -> Result<Vec<StaticRouteRule>, String> {
    let config = config::load_config(&app_handle);
    Ok(config.route_rules)
}

#[tauri::command]
pub async fn add_route_rule(
    app_handle: tauri::AppHandle,
    image_pattern: String,
    port_mappings: Vec<crate::models::PortMapping>,
) -> Result<Vec<StaticRouteRule>, String> {
    let mut app_config = config::load_config(&app_handle);

    if app_config
        .route_rules
        .iter()
        .any(|r| r.image_pattern == image_pattern)
    {
        return Err(format!(
            "Route rule for image '{image_pattern}' already exists"
        ));
    }

    app_config.route_rules.push(StaticRouteRule {
        image_pattern,
        port_mappings,
    });

    config::save_config(&app_handle, &app_config)?;
    Ok(app_config.route_rules)
}

#[tauri::command]
pub async fn remove_route_rule(
    app_handle: tauri::AppHandle,
    image_pattern: String,
) -> Result<Vec<StaticRouteRule>, String> {
    let mut app_config = config::load_config(&app_handle);
    app_config
        .route_rules
        .retain(|r| r.image_pattern != image_pattern);
    config::save_config(&app_handle, &app_config)?;
    Ok(app_config.route_rules)
}

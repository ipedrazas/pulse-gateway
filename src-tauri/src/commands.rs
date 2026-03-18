use tauri::{Emitter, State};

use crate::caddy;
use crate::config;
use crate::credentials;
use crate::docker;
use crate::models::{AppConfig, CaddyStatus, CertInfo, DnsProvider, EnvVarEntry, Gateway, LogEntry, StaticRouteRule};
use crate::watcher;
use crate::AppState;

/// Resolve env var entries from config to (key, value) pairs using keyring.
fn resolve_env_vars(config: &AppConfig) -> Vec<(String, String)> {
    eprintln!("[resolve_env_vars] config has {} env var entries", config.caddy_env_vars.len());
    let result: Vec<(String, String)> = config
        .caddy_env_vars
        .iter()
        .filter_map(|entry| {
            match credentials::get_value(&entry.key) {
                Ok(val) => {
                    eprintln!("[resolve_env_vars] resolved '{}' (len={})", entry.key, val.len());
                    Some((entry.key.clone(), val))
                }
                Err(e) => {
                    eprintln!("[resolve_env_vars] FAILED to resolve '{}': {e}", entry.key);
                    None
                }
            }
        })
        .collect();
    eprintln!("[resolve_env_vars] resolved {} of {} env vars", result.len(), config.caddy_env_vars.len());
    result
}

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
    let env_vars = resolve_env_vars(&config);

    if let Err(e) = docker::ensure_network(&state.docker).await {
        return Ok(CaddyStatus {
            running: false,
            api_reachable: false,
            error: Some(e),
        });
    }

    if let Err(e) = docker::ensure_caddy(&state.docker, &config.caddy_image, &env_vars).await {
        return Ok(CaddyStatus {
            running: false,
            api_reachable: false,
            error: Some(e),
        });
    }

    let mut api_ready = false;
    for _ in 0..10 {
        if caddy::check_health(&state.http_client).await {
            api_ready = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    if api_ready {
        let auto = state.auto_gateways.lock().await;
        let combined = watcher::combine_routes(&config.static_routes, &auto);
        let _ = caddy::push_routes(&state.http_client, &combined, &config.domain, &config.dns_provider).await;
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
pub async fn stop_caddy(
    state: State<'_, AppState>,
) -> Result<CaddyStatus, String> {
    if let Err(e) = docker::stop_caddy(&state.docker).await {
        return Ok(CaddyStatus {
            running: true,
            api_reachable: true,
            error: Some(e),
        });
    }

    Ok(CaddyStatus {
        running: false,
        api_reachable: false,
        error: None,
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

    let auto = state.auto_gateways.lock().await;
    let combined = watcher::combine_routes(&app_config.static_routes, &auto);
    let _ = caddy::push_routes(&state.http_client, &combined, &app_config.domain, &app_config.dns_provider).await;

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

    let auto = state.auto_gateways.lock().await;
    let combined = watcher::combine_routes(&app_config.static_routes, &auto);
    let _ = caddy::push_routes(&state.http_client, &combined, &app_config.domain, &app_config.dns_provider).await;

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
    dns_provider: DnsProvider,
) -> Result<AppConfig, String> {
    let mut app_config = config::load_config(&app_handle);
    app_config.domain = domain;
    app_config.caddy_image = caddy_image;
    app_config.dns_provider = dns_provider;
    config::save_config(&app_handle, &app_config)?;
    Ok(app_config)
}

/// Get the list of env var keys and whether each has a stored value.
#[tauri::command]
pub async fn get_env_vars(
    app_handle: tauri::AppHandle,
) -> Result<Vec<(String, bool)>, String> {
    let config = config::load_config(&app_handle);
    let result = config
        .caddy_env_vars
        .iter()
        .map(|entry| (entry.key.clone(), credentials::has_value(&entry.key)))
        .collect();
    Ok(result)
}

/// Save an env var: stores the key in config and the value in keyring.
/// If value is empty, removes the credential but keeps the key.
#[tauri::command]
pub async fn save_env_var(
    app_handle: tauri::AppHandle,
    key: String,
    value: String,
) -> Result<Vec<(String, bool)>, String> {
    let mut app_config = config::load_config(&app_handle);

    // Add key to config if not already present
    if !app_config.caddy_env_vars.iter().any(|e| e.key == key) {
        app_config.caddy_env_vars.push(EnvVarEntry { key: key.clone() });
        config::save_config(&app_handle, &app_config)?;
    }

    // Store value in keyring/file
    if !value.is_empty() {
        eprintln!("[save_env_var] storing value for key='{key}', value_len={}", value.len());
        credentials::store_value(&key, &value)?;
    } else {
        eprintln!("[save_env_var] value is empty for key='{key}', skipping store");
    }

    // Verify it was stored
    let stored = credentials::has_value(&key);
    eprintln!("[save_env_var] has_value('{key}') = {stored}");
    if stored {
        if let Ok(v) = credentials::get_value(&key) {
            eprintln!("[save_env_var] get_value('{key}') = '{}...' (len={})", &v[..v.len().min(4)], v.len());
        }
    }

    // Return updated list
    let result = app_config
        .caddy_env_vars
        .iter()
        .map(|entry| {
            let has = credentials::has_value(&entry.key);
            eprintln!("[save_env_var] returning key='{}', has_value={}", entry.key, has);
            (entry.key.clone(), has)
        })
        .collect();
    Ok(result)
}

/// Remove an env var: removes from config and keyring.
#[tauri::command]
pub async fn remove_env_var(
    app_handle: tauri::AppHandle,
    key: String,
) -> Result<Vec<(String, bool)>, String> {
    let mut app_config = config::load_config(&app_handle);
    app_config.caddy_env_vars.retain(|e| e.key != key);
    config::save_config(&app_handle, &app_config)?;
    credentials::delete_value(&key);

    let result = app_config
        .caddy_env_vars
        .iter()
        .map(|entry| (entry.key.clone(), credentials::has_value(&entry.key)))
        .collect();
    Ok(result)
}

#[tauri::command]
pub async fn get_cert_info(
    app_handle: tauri::AppHandle,
) -> Result<CertInfo, String> {
    let config = config::load_config(&app_handle);
    let has_env_vars = !config.caddy_env_vars.is_empty();
    Ok(caddy::get_cert_info(&config.domain, has_env_vars))
}

#[tauri::command]
pub async fn get_event_log(state: State<'_, AppState>) -> Result<Vec<LogEntry>, String> {
    let log = state.event_log.lock().await;
    Ok(log.clone())
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

use std::collections::HashMap;
use std::sync::Arc;

use bollard::system::EventsOptions;
use bollard::Docker;
use futures::StreamExt;
use reqwest::Client;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

use crate::caddy;
use crate::config;
use crate::docker::{self, CADDY_CONTAINER_NAME};
use crate::models::{Gateway, GatewaySource, LogEntry, StaticRouteRule};

const MAX_LOG_ENTRIES: usize = 200;

pub async fn run(
    docker: Docker,
    http_client: Client,
    auto_gateways: Arc<Mutex<Vec<Gateway>>>,
    event_log: Arc<Mutex<Vec<LogEntry>>>,
    app_handle: AppHandle,
) {
    log_event(&event_log, &app_handle, "info", "Pulse Gateway watcher started");

    if let Err(e) = reconcile(&docker, &http_client, &auto_gateways, &event_log, &app_handle).await {
        log_event(&event_log, &app_handle, "error", &format!("Reconciliation failed: {e}"));
    }

    loop {
        if let Err(e) =
            listen_events(&docker, &http_client, &auto_gateways, &event_log, &app_handle).await
        {
            log_event(&event_log, &app_handle, "error", &format!("Event stream error: {e}, reconnecting..."));
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }
}

async fn reconcile(
    docker: &Docker,
    http_client: &Client,
    auto_gateways: &Arc<Mutex<Vec<Gateway>>>,
    event_log: &Arc<Mutex<Vec<LogEntry>>>,
    app_handle: &AppHandle,
) -> Result<(), String> {
    let container_ids = docker::list_running_containers(docker).await?;
    let app_config = config::load_config(app_handle);

    log_event(event_log, app_handle, "info", &format!("Reconciling {} running containers", container_ids.len()));

    let mut gateways = auto_gateways.lock().await;
    gateways.clear();

    for id in container_ids {
        if let Ok(info) = docker::inspect_for_routing(docker, &id).await {
            if should_skip(&info) {
                continue;
            }

            if !info.on_network {
                let _ = docker::attach_to_network(docker, &id).await;
            }

            let new_routes =
                build_auto_routes(&info, &app_config.route_rules, &gateways, &app_config.static_routes);

            for route in &new_routes {
                log_event(
                    event_log,
                    app_handle,
                    "info",
                    &format!("Discovered container '{}' → {}.{}", info.name, route.subdomain, app_config.domain),
                );
            }

            gateways.extend(new_routes);
        }
    }

    let combined = combine_routes(&app_config.static_routes, &gateways);
    let _ = caddy::push_routes(http_client, &combined, &app_config.domain).await;

    log_event(
        event_log,
        app_handle,
        "info",
        &format!("Reconciliation complete: {} total routes ({} auto, {} static)", combined.len(), gateways.len(), app_config.static_routes.len()),
    );

    emit_gateways(app_handle, &combined);
    Ok(())
}

async fn listen_events(
    docker: &Docker,
    http_client: &Client,
    auto_gateways: &Arc<Mutex<Vec<Gateway>>>,
    event_log: &Arc<Mutex<Vec<LogEntry>>>,
    app_handle: &AppHandle,
) -> Result<(), String> {
    let filters: HashMap<String, Vec<String>> = HashMap::from([
        ("type".to_string(), vec!["container".to_string()]),
        (
            "event".to_string(),
            vec![
                "start".to_string(),
                "stop".to_string(),
                "die".to_string(),
            ],
        ),
    ]);

    let opts = EventsOptions {
        filters,
        ..Default::default()
    };

    let mut stream = docker.events(Some(opts));

    while let Some(event) = stream.next().await {
        let ev = event.map_err(|e| format!("Docker event error: {e}"))?;
        let action = ev.action.as_deref().unwrap_or("");
        let container_id = ev
            .actor
            .as_ref()
            .and_then(|a| a.id.as_deref())
            .unwrap_or("");

        if container_id.is_empty() {
            continue;
        }

        match action {
            "start" => {
                handle_start(docker, http_client, auto_gateways, event_log, app_handle, container_id).await;
            }
            "stop" | "die" => {
                handle_stop(http_client, auto_gateways, event_log, app_handle, container_id, action).await;
            }
            _ => {}
        }
    }

    Err("Event stream ended".to_string())
}

async fn handle_start(
    docker: &Docker,
    http_client: &Client,
    auto_gateways: &Arc<Mutex<Vec<Gateway>>>,
    event_log: &Arc<Mutex<Vec<LogEntry>>>,
    app_handle: &AppHandle,
    container_id: &str,
) {
    let info = match docker::inspect_for_routing(docker, container_id).await {
        Ok(info) => info,
        Err(e) => {
            log_event(event_log, app_handle, "error", &format!("Failed to inspect container {}: {e}", &container_id[..12.min(container_id.len())]));
            return;
        }
    };

    // If Caddy just restarted, re-push all routes
    if info.name == CADDY_CONTAINER_NAME {
        log_event(event_log, app_handle, "info", "Caddy container restarted, re-pushing routes");
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let app_config = config::load_config(app_handle);
        let gateways = auto_gateways.lock().await;
        let combined = combine_routes(&app_config.static_routes, &gateways);
        let _ = caddy::push_routes(http_client, &combined, &app_config.domain).await;
        return;
    }

    if should_skip(&info) {
        return;
    }

    log_event(event_log, app_handle, "info", &format!("Detected new container: '{}'", info.name));

    if !info.on_network {
        if let Err(e) = docker::attach_to_network(docker, container_id).await {
            log_event(event_log, app_handle, "error", &format!("Failed to attach '{}' to network: {e}", info.name));
        }
    }

    let app_config = config::load_config(app_handle);
    let mut gateways = auto_gateways.lock().await;

    let new_routes =
        build_auto_routes(&info, &app_config.route_rules, &gateways, &app_config.static_routes);

    if new_routes.is_empty() {
        log_event(event_log, app_handle, "warn", &format!("Container '{}' has no exposed ports, skipping", info.name));
        return;
    }

    for route in &new_routes {
        log_event(
            event_log,
            app_handle,
            "info",
            &format!("Route added: {}.{} → {}:{}", route.subdomain, app_config.domain, route.target_host, route.port),
        );
    }

    gateways.extend(new_routes);

    let combined = combine_routes(&app_config.static_routes, &gateways);
    let _ = caddy::push_routes(http_client, &combined, &app_config.domain).await;

    emit_gateways(app_handle, &combined);
}

async fn handle_stop(
    http_client: &Client,
    auto_gateways: &Arc<Mutex<Vec<Gateway>>>,
    event_log: &Arc<Mutex<Vec<LogEntry>>>,
    app_handle: &AppHandle,
    container_id: &str,
    action: &str,
) {
    let mut gateways = auto_gateways.lock().await;
    let before = gateways.len();

    // Collect route info before removing for logging
    let removed: Vec<String> = gateways
        .iter()
        .filter(|g| g.container_id.as_deref() == Some(container_id))
        .map(|g| {
            let name = g.container_name.as_deref().unwrap_or("unknown");
            format!("{} ({})", g.subdomain, name)
        })
        .collect();

    gateways.retain(|g| g.container_id.as_deref() != Some(container_id));

    if gateways.len() == before {
        return;
    }

    for desc in &removed {
        log_event(
            event_log,
            app_handle,
            "info",
            &format!("Route removed: {} (container {})", desc, action),
        );
    }

    let app_config = config::load_config(app_handle);
    let combined = combine_routes(&app_config.static_routes, &gateways);
    let _ = caddy::push_routes(http_client, &combined, &app_config.domain).await;

    emit_gateways(app_handle, &combined);
}

fn should_skip(info: &docker::ContainerInfo) -> bool {
    if info.name == CADDY_CONTAINER_NAME {
        return true;
    }
    if info.labels.get("pulse.proxy").map(|v| v.as_str()) == Some("false") {
        return true;
    }
    false
}

fn build_auto_routes(
    info: &docker::ContainerInfo,
    route_rules: &[StaticRouteRule],
    existing_auto: &[Gateway],
    static_routes: &[Gateway],
) -> Vec<Gateway> {
    let ports = resolve_ports(info, route_rules);

    if ports.is_empty() {
        return Vec::new();
    }

    let mut routes = Vec::new();

    for (port, subdomain_override) in &ports {
        let base_subdomain = if let Some(tmpl) = subdomain_override {
            tmpl.replace("{name}", &info.name)
        } else if ports.len() == 1 {
            info.name.clone()
        } else {
            format!("{}-{}", info.name, port)
        };

        let subdomain = resolve_collision(&base_subdomain, existing_auto, static_routes, &routes);

        routes.push(Gateway {
            subdomain,
            target_host: info.name.clone(),
            port: *port,
            source: GatewaySource::Auto,
            container_id: Some(info.id.clone()),
            container_name: Some(info.name.clone()),
        });
    }

    routes
}

fn resolve_ports(
    info: &docker::ContainerInfo,
    route_rules: &[StaticRouteRule],
) -> Vec<(u16, Option<String>)> {
    if let Some(port_str) = info.labels.get("pulse.port") {
        if let Ok(port) = port_str.parse::<u16>() {
            let subdomain = info.labels.get("pulse.subdomain").cloned();
            return vec![(port, subdomain)];
        }
    }

    for rule in route_rules {
        if image_matches(&info.image, &rule.image_pattern) {
            return rule
                .port_mappings
                .iter()
                .map(|pm| (pm.port, Some(pm.subdomain_template.clone())))
                .collect();
        }
    }

    info.ports.iter().map(|p| (*p, None)).collect()
}

fn image_matches(image: &str, pattern: &str) -> bool {
    if pattern.ends_with('*') {
        let prefix = &pattern[..pattern.len() - 1];
        image.starts_with(prefix)
    } else {
        image == pattern
    }
}

fn resolve_collision(
    base: &str,
    existing_auto: &[Gateway],
    static_routes: &[Gateway],
    pending: &[Gateway],
) -> String {
    let is_taken = |name: &str| -> bool {
        existing_auto.iter().any(|g| g.subdomain == name)
            || static_routes.iter().any(|g| g.subdomain == name)
            || pending.iter().any(|g| g.subdomain == name)
    };

    if !is_taken(base) {
        return base.to_string();
    }

    let mut counter = 2;
    loop {
        let candidate = format!("{base}-{counter}");
        if !is_taken(&candidate) {
            return candidate;
        }
        counter += 1;
        if counter > 100 {
            return format!("{base}-{}", uuid_short());
        }
    }
}

fn uuid_short() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{:x}", t & 0xFFFF)
}

pub fn combine_routes(static_routes: &[Gateway], auto_routes: &[Gateway]) -> Vec<Gateway> {
    let mut combined = static_routes.to_vec();
    combined.extend(auto_routes.iter().cloned());
    combined
}

fn emit_gateways(app_handle: &AppHandle, gateways: &[Gateway]) {
    let _ = app_handle.emit("gateways-changed", gateways.to_vec());
}

fn log_event(
    event_log: &Arc<Mutex<Vec<LogEntry>>>,
    app_handle: &AppHandle,
    level: &str,
    message: &str,
) {
    let entry = LogEntry {
        timestamp: chrono_now(),
        level: level.to_string(),
        message: message.to_string(),
    };

    let _ = app_handle.emit("log-entry", &entry);

    // Also store in memory (fire-and-forget with try_lock to avoid blocking)
    if let Ok(mut log) = event_log.try_lock() {
        log.push(entry);
        let len = log.len();
        if len > MAX_LOG_ENTRIES {
            log.drain(0..len - MAX_LOG_ENTRIES);
        }
    }
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Simple ISO-ish format from epoch seconds
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, mins, s)
}

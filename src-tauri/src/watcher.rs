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
use crate::models::{Gateway, GatewaySource, StaticRouteRule};

/// Run the Docker event watcher. This is a long-running task that:
/// 1. Reconciles existing containers on startup
/// 2. Listens for container start/stop/die events
/// 3. Monitors Caddy container health
pub async fn run(
    docker: Docker,
    http_client: Client,
    auto_gateways: Arc<Mutex<Vec<Gateway>>>,
    app_handle: AppHandle,
) {
    // Initial reconciliation
    if let Err(e) = reconcile(&docker, &http_client, &auto_gateways, &app_handle).await {
        eprintln!("[watcher] reconciliation failed: {e}");
    }

    // Listen to Docker events (reconnects on error)
    loop {
        if let Err(e) =
            listen_events(&docker, &http_client, &auto_gateways, &app_handle).await
        {
            eprintln!("[watcher] event stream error: {e}, reconnecting in 2s...");
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }
}

/// Scan all running containers and create auto-routes for eligible ones.
async fn reconcile(
    docker: &Docker,
    http_client: &Client,
    auto_gateways: &Arc<Mutex<Vec<Gateway>>>,
    app_handle: &AppHandle,
) -> Result<(), String> {
    let container_ids = docker::list_running_containers(docker).await?;
    let app_config = config::load_config(app_handle);

    let mut gateways = auto_gateways.lock().await;
    gateways.clear();

    for id in container_ids {
        if let Ok(info) = docker::inspect_for_routing(docker, &id).await {
            if should_skip(&info) {
                continue;
            }

            // Attach to network if needed
            if !info.on_network {
                let _ = docker::attach_to_network(docker, &id).await;
            }

            let new_routes =
                build_auto_routes(&info, &app_config.route_rules, &gateways, &app_config.static_routes);
            gateways.extend(new_routes);
        }
    }

    // Push combined routes to Caddy
    let combined = combine_routes(&app_config.static_routes, &gateways);
    let _ = caddy::push_routes(http_client, &combined, &app_config.domain).await;

    // Emit to frontend
    emit_gateways(app_handle, &combined);

    Ok(())
}

/// Subscribe to Docker events and process container start/stop/die.
async fn listen_events(
    docker: &Docker,
    http_client: &Client,
    auto_gateways: &Arc<Mutex<Vec<Gateway>>>,
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
                handle_start(docker, http_client, auto_gateways, app_handle, container_id).await;
            }
            "stop" | "die" => {
                handle_stop(http_client, auto_gateways, app_handle, container_id).await;
            }
            _ => {}
        }
    }

    Err("Event stream ended".to_string())
}

/// Handle a container start event.
async fn handle_start(
    docker: &Docker,
    http_client: &Client,
    auto_gateways: &Arc<Mutex<Vec<Gateway>>>,
    app_handle: &AppHandle,
    container_id: &str,
) {
    let info = match docker::inspect_for_routing(docker, container_id).await {
        Ok(info) => info,
        Err(e) => {
            eprintln!("[watcher] inspect failed for {container_id}: {e}");
            return;
        }
    };

    // If Caddy just restarted, re-push all routes
    if info.name == CADDY_CONTAINER_NAME {
        eprintln!("[watcher] Caddy container restarted, re-pushing routes");
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

    // Attach to network
    if !info.on_network {
        if let Err(e) = docker::attach_to_network(docker, container_id).await {
            eprintln!("[watcher] network attach failed for {}: {e}", info.name);
        }
    }

    let app_config = config::load_config(app_handle);
    let mut gateways = auto_gateways.lock().await;

    let new_routes =
        build_auto_routes(&info, &app_config.route_rules, &gateways, &app_config.static_routes);

    if new_routes.is_empty() {
        return;
    }

    for route in &new_routes {
        eprintln!(
            "[watcher] auto-route: {}.{} -> {}:{}",
            route.subdomain, app_config.domain, route.target_host, route.port
        );
    }

    gateways.extend(new_routes);

    // Push combined routes
    let combined = combine_routes(&app_config.static_routes, &gateways);
    let _ = caddy::push_routes(http_client, &combined, &app_config.domain).await;

    emit_gateways(app_handle, &combined);
}

/// Handle a container stop/die event.
async fn handle_stop(
    http_client: &Client,
    auto_gateways: &Arc<Mutex<Vec<Gateway>>>,
    app_handle: &AppHandle,
    container_id: &str,
) {
    let mut gateways = auto_gateways.lock().await;
    let before = gateways.len();
    gateways.retain(|g| g.container_id.as_deref() != Some(container_id));

    if gateways.len() == before {
        // No routes were removed — container wasn't tracked
        return;
    }

    eprintln!("[watcher] removed routes for container {container_id}");

    let app_config = config::load_config(app_handle);
    let combined = combine_routes(&app_config.static_routes, &gateways);
    let _ = caddy::push_routes(http_client, &combined, &app_config.domain).await;

    emit_gateways(app_handle, &combined);
}

/// Check if a container should be skipped for auto-routing.
fn should_skip(info: &docker::ContainerInfo) -> bool {
    // Skip the Caddy sidecar
    if info.name == CADDY_CONTAINER_NAME {
        return true;
    }

    // Skip if opted out
    if info.labels.get("pulse.proxy").map(|v| v.as_str()) == Some("false") {
        return true;
    }

    false
}

/// Build auto-route Gateway entries for a container.
fn build_auto_routes(
    info: &docker::ContainerInfo,
    route_rules: &[StaticRouteRule],
    existing_auto: &[Gateway],
    static_routes: &[Gateway],
) -> Vec<Gateway> {
    // Determine ports: pulse.port label > route rule > EXPOSE
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

        if subdomain != base_subdomain {
            eprintln!(
                "[watcher] name collision: '{}' -> '{}' for container {}",
                base_subdomain, subdomain, info.name
            );
        }

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

/// Resolve ports for a container following the priority:
/// 1. pulse.port label (explicit)
/// 2. Static route rule matching the image
/// 3. EXPOSE directives
///
/// Returns (port, optional subdomain template override).
fn resolve_ports(
    info: &docker::ContainerInfo,
    route_rules: &[StaticRouteRule],
) -> Vec<(u16, Option<String>)> {
    // Priority 1: pulse.port label
    if let Some(port_str) = info.labels.get("pulse.port") {
        if let Ok(port) = port_str.parse::<u16>() {
            // Also check pulse.subdomain for an explicit name
            let subdomain = info.labels.get("pulse.subdomain").cloned();
            return vec![(port, subdomain)];
        }
    }

    // Priority 2: image-based static route rules
    for rule in route_rules {
        if image_matches(&info.image, &rule.image_pattern) {
            return rule
                .port_mappings
                .iter()
                .map(|pm| (pm.port, Some(pm.subdomain_template.clone())))
                .collect();
        }
    }

    // Priority 3: EXPOSE ports
    info.ports.iter().map(|p| (*p, None)).collect()
}

/// Simple glob matching for image patterns.
/// Supports trailing `*` wildcard (e.g., `postgres:*` matches `postgres:16`).
fn image_matches(image: &str, pattern: &str) -> bool {
    if pattern.ends_with('*') {
        let prefix = &pattern[..pattern.len() - 1];
        image.starts_with(prefix)
    } else {
        image == pattern
    }
}

/// Resolve subdomain name collisions by appending -2, -3, etc.
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
            // Safety valve
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

/// Combine static and auto routes into a single list.
pub fn combine_routes(static_routes: &[Gateway], auto_routes: &[Gateway]) -> Vec<Gateway> {
    let mut combined = static_routes.to_vec();
    combined.extend(auto_routes.iter().cloned());
    combined
}

/// Emit the full gateway list to the frontend.
fn emit_gateways(app_handle: &AppHandle, gateways: &[Gateway]) {
    let _ = app_handle.emit("gateways-changed", gateways.to_vec());
}

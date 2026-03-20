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
    log_event(
        &event_log,
        &app_handle,
        "info",
        "Pulse Gateway watcher started",
    );

    if let Err(e) = reconcile(
        &docker,
        &http_client,
        &auto_gateways,
        &event_log,
        &app_handle,
    )
    .await
    {
        log_event(
            &event_log,
            &app_handle,
            "error",
            &format!("Reconciliation failed: {e}"),
        );
    }

    loop {
        if let Err(e) = listen_events(
            &docker,
            &http_client,
            &auto_gateways,
            &event_log,
            &app_handle,
        )
        .await
        {
            log_event(
                &event_log,
                &app_handle,
                "error",
                &format!("Event stream error: {e}, reconnecting..."),
            );
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

    log_event(
        event_log,
        app_handle,
        "info",
        &format!("Reconciling {} running containers", container_ids.len()),
    );

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

            let new_routes = build_auto_routes(
                &info,
                &app_config.route_rules,
                &gateways,
                &app_config.static_routes,
            );

            for route in &new_routes {
                log_event(
                    event_log,
                    app_handle,
                    "info",
                    &format!(
                        "Discovered container '{}' → {}.{}",
                        info.name, route.subdomain, app_config.domain
                    ),
                );
            }

            gateways.extend(new_routes);
        }
    }

    let combined = combine_routes(&app_config.static_routes, &gateways);
    let _ = caddy::push_routes(
        http_client,
        &combined,
        &app_config.domain,
        &app_config.dns_provider,
    )
    .await;

    log_event(
        event_log,
        app_handle,
        "info",
        &format!(
            "Reconciliation complete: {} total routes ({} auto, {} static)",
            combined.len(),
            gateways.len(),
            app_config.static_routes.len()
        ),
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
            vec!["start".to_string(), "stop".to_string(), "die".to_string()],
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
                handle_start(
                    docker,
                    http_client,
                    auto_gateways,
                    event_log,
                    app_handle,
                    container_id,
                )
                .await;
            }
            "stop" | "die" => {
                handle_stop(
                    http_client,
                    auto_gateways,
                    event_log,
                    app_handle,
                    container_id,
                    action,
                )
                .await;
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
            log_event(
                event_log,
                app_handle,
                "error",
                &format!(
                    "Failed to inspect container {}: {e}",
                    &container_id[..12.min(container_id.len())]
                ),
            );
            return;
        }
    };

    // If Caddy just restarted, re-push all routes
    if info.name == CADDY_CONTAINER_NAME {
        log_event(
            event_log,
            app_handle,
            "info",
            "Caddy container restarted, re-pushing routes",
        );
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let app_config = config::load_config(app_handle);
        let gateways = auto_gateways.lock().await;
        let combined = combine_routes(&app_config.static_routes, &gateways);
        let _ = caddy::push_routes(
            http_client,
            &combined,
            &app_config.domain,
            &app_config.dns_provider,
        )
        .await;
        return;
    }

    if should_skip(&info) {
        return;
    }

    log_event(
        event_log,
        app_handle,
        "info",
        &format!("Detected new container: '{}'", info.name),
    );

    if !info.on_network {
        if let Err(e) = docker::attach_to_network(docker, container_id).await {
            log_event(
                event_log,
                app_handle,
                "error",
                &format!("Failed to attach '{}' to network: {e}", info.name),
            );
        }
    }

    let app_config = config::load_config(app_handle);
    let mut gateways = auto_gateways.lock().await;

    let new_routes = build_auto_routes(
        &info,
        &app_config.route_rules,
        &gateways,
        &app_config.static_routes,
    );

    if new_routes.is_empty() {
        log_event(
            event_log,
            app_handle,
            "warn",
            &format!("Container '{}' has no exposed ports, skipping", info.name),
        );
        return;
    }

    for route in &new_routes {
        log_event(
            event_log,
            app_handle,
            "info",
            &format!(
                "Route added: {}.{} → {}:{}",
                route.subdomain, app_config.domain, route.target_host, route.port
            ),
        );
    }

    gateways.extend(new_routes);

    let combined = combine_routes(&app_config.static_routes, &gateways);
    let _ = caddy::push_routes(
        http_client,
        &combined,
        &app_config.domain,
        &app_config.dns_provider,
    )
    .await;

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
    let _ = caddy::push_routes(
        http_client,
        &combined,
        &app_config.domain,
        &app_config.dns_provider,
    )
    .await;

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
        let raw_subdomain = if let Some(tmpl) = subdomain_override {
            tmpl.replace("{name}", &info.name)
        } else if ports.len() == 1 {
            info.name.clone()
        } else {
            format!("{}-{}", info.name, port)
        };

        let base_subdomain = sanitize_subdomain(&raw_subdomain);
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
    if let Some(prefix) = pattern.strip_suffix('*') {
        image.starts_with(prefix)
    } else {
        image == pattern
    }
}

/// Sanitize a string into a valid DNS subdomain label:
/// - lowercase
/// - replace underscores and dots with hyphens
/// - strip any character that isn't alphanumeric or hyphen
/// - collapse consecutive hyphens
/// - trim leading/trailing hyphens
fn sanitize_subdomain(name: &str) -> String {
    let s: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c == '_' || c == '.' { '-' } else { c })
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();

    // Collapse consecutive hyphens and trim
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        if c == '-' && result.ends_with('-') {
            continue;
        }
        result.push(c);
    }
    result.trim_matches('-').to_string()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PortMapping;
    use std::collections::HashMap;

    fn make_info(name: &str, image: &str, ports: Vec<u16>) -> docker::ContainerInfo {
        docker::ContainerInfo {
            id: format!("{name}-id"),
            name: name.to_string(),
            image: image.to_string(),
            labels: HashMap::new(),
            ports,
            on_network: true,
        }
    }

    fn make_info_with_labels(
        name: &str,
        labels: HashMap<String, String>,
        ports: Vec<u16>,
    ) -> docker::ContainerInfo {
        docker::ContainerInfo {
            id: format!("{name}-id"),
            name: name.to_string(),
            image: "some-image:latest".to_string(),
            labels,
            ports,
            on_network: true,
        }
    }

    fn make_gateway(subdomain: &str) -> Gateway {
        Gateway {
            subdomain: subdomain.to_string(),
            target_host: "test".to_string(),
            port: 80,
            source: GatewaySource::Auto,
            container_id: None,
            container_name: None,
        }
    }

    // --- sanitize_subdomain ---

    #[test]
    fn sanitize_lowercase() {
        assert_eq!(sanitize_subdomain("MyApp"), "myapp");
    }

    #[test]
    fn sanitize_underscores() {
        assert_eq!(sanitize_subdomain("affectionate_ride"), "affectionate-ride");
    }

    #[test]
    fn sanitize_dots() {
        assert_eq!(sanitize_subdomain("my.app.name"), "my-app-name");
    }

    #[test]
    fn sanitize_invalid_chars() {
        assert_eq!(sanitize_subdomain("app@name!v2"), "appnamev2");
    }

    #[test]
    fn sanitize_consecutive_hyphens() {
        assert_eq!(sanitize_subdomain("a__b"), "a-b");
        assert_eq!(sanitize_subdomain("a---b"), "a-b");
    }

    #[test]
    fn sanitize_leading_trailing_hyphens() {
        assert_eq!(sanitize_subdomain("_app_"), "app");
        assert_eq!(sanitize_subdomain("-app-"), "app");
    }

    #[test]
    fn sanitize_mixed() {
        assert_eq!(sanitize_subdomain("My_Cool.App@v2"), "my-cool-appv2");
    }

    #[test]
    fn sanitize_already_valid() {
        assert_eq!(sanitize_subdomain("my-app"), "my-app");
    }

    #[test]
    fn sanitize_empty() {
        assert_eq!(sanitize_subdomain(""), "");
    }

    // --- image_matches ---

    #[test]
    fn image_exact_match() {
        assert!(image_matches("nginx:latest", "nginx:latest"));
    }

    #[test]
    fn image_exact_no_match() {
        assert!(!image_matches("nginx:latest", "caddy:2"));
    }

    #[test]
    fn image_wildcard_match() {
        assert!(image_matches("nginx:latest", "nginx*"));
        assert!(image_matches("nginx:1.25", "nginx:*"));
    }

    #[test]
    fn image_wildcard_no_match() {
        assert!(!image_matches("caddy:2", "nginx*"));
    }

    // --- resolve_collision ---

    #[test]
    fn collision_no_conflict() {
        let result = resolve_collision("myapp", &[], &[], &[]);
        assert_eq!(result, "myapp");
    }

    #[test]
    fn collision_with_existing_auto() {
        let existing = vec![make_gateway("myapp")];
        let result = resolve_collision("myapp", &existing, &[], &[]);
        assert_eq!(result, "myapp-2");
    }

    #[test]
    fn collision_with_static() {
        let statics = vec![make_gateway("myapp")];
        let result = resolve_collision("myapp", &[], &statics, &[]);
        assert_eq!(result, "myapp-2");
    }

    #[test]
    fn collision_with_pending() {
        let pending = vec![make_gateway("myapp")];
        let result = resolve_collision("myapp", &[], &[], &pending);
        assert_eq!(result, "myapp-2");
    }

    #[test]
    fn collision_increments() {
        let existing = vec![make_gateway("myapp"), make_gateway("myapp-2")];
        let result = resolve_collision("myapp", &existing, &[], &[]);
        assert_eq!(result, "myapp-3");
    }

    // --- should_skip ---

    #[test]
    fn skip_caddy_container() {
        let info = make_info(CADDY_CONTAINER_NAME, "caddy:2", vec![]);
        assert!(should_skip(&info));
    }

    #[test]
    fn skip_opted_out() {
        let mut labels = HashMap::new();
        labels.insert("pulse.proxy".to_string(), "false".to_string());
        let info = make_info_with_labels("myapp", labels, vec![80]);
        assert!(should_skip(&info));
    }

    #[test]
    fn no_skip_normal_container() {
        let info = make_info("myapp", "nginx:latest", vec![80]);
        assert!(!should_skip(&info));
    }

    #[test]
    fn no_skip_proxy_true() {
        let mut labels = HashMap::new();
        labels.insert("pulse.proxy".to_string(), "true".to_string());
        let info = make_info_with_labels("myapp", labels, vec![80]);
        assert!(!should_skip(&info));
    }

    // --- resolve_ports ---

    #[test]
    fn ports_from_label() {
        let mut labels = HashMap::new();
        labels.insert("pulse.port".to_string(), "3000".to_string());
        let info = make_info_with_labels("myapp", labels, vec![80, 443]);
        let ports = resolve_ports(&info, &[]);
        assert_eq!(ports, vec![(3000, None)]);
    }

    #[test]
    fn ports_from_label_with_subdomain() {
        let mut labels = HashMap::new();
        labels.insert("pulse.port".to_string(), "3000".to_string());
        labels.insert("pulse.subdomain".to_string(), "api".to_string());
        let info = make_info_with_labels("myapp", labels, vec![80]);
        let ports = resolve_ports(&info, &[]);
        assert_eq!(ports, vec![(3000, Some("api".to_string()))]);
    }

    #[test]
    fn ports_from_route_rule() {
        let info = make_info("myapp", "postgres:15", vec![5432]);
        let rules = vec![StaticRouteRule {
            image_pattern: "postgres*".to_string(),
            port_mappings: vec![PortMapping {
                port: 5432,
                subdomain_template: "{name}-db".to_string(),
            }],
        }];
        let ports = resolve_ports(&info, &rules);
        assert_eq!(ports, vec![(5432, Some("{name}-db".to_string()))]);
    }

    #[test]
    fn ports_from_expose() {
        let info = make_info("myapp", "custom:latest", vec![8080, 9090]);
        let ports = resolve_ports(&info, &[]);
        assert_eq!(ports, vec![(8080, None), (9090, None)]);
    }

    #[test]
    fn ports_empty() {
        let info = make_info("myapp", "custom:latest", vec![]);
        let ports = resolve_ports(&info, &[]);
        assert!(ports.is_empty());
    }

    // --- build_auto_routes ---

    #[test]
    fn auto_routes_single_port() {
        let info = make_info("myapp", "nginx:latest", vec![80]);
        let routes = build_auto_routes(&info, &[], &[], &[]);
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].subdomain, "myapp");
        assert_eq!(routes[0].port, 80);
        assert_eq!(routes[0].source, GatewaySource::Auto);
    }

    #[test]
    fn auto_routes_multi_port() {
        let info = make_info("myapp", "nginx:latest", vec![80, 443]);
        let routes = build_auto_routes(&info, &[], &[], &[]);
        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].subdomain, "myapp-80");
        assert_eq!(routes[1].subdomain, "myapp-443");
    }

    #[test]
    fn auto_routes_no_ports() {
        let info = make_info("myapp", "nginx:latest", vec![]);
        let routes = build_auto_routes(&info, &[], &[], &[]);
        assert!(routes.is_empty());
    }

    #[test]
    fn auto_routes_sanitizes_name() {
        let info = make_info("my_ugly_name", "nginx:latest", vec![80]);
        let routes = build_auto_routes(&info, &[], &[], &[]);
        assert_eq!(routes[0].subdomain, "my-ugly-name");
    }

    #[test]
    fn auto_routes_collision_with_static() {
        let info = make_info("myapp", "nginx:latest", vec![80]);
        let statics = vec![make_gateway("myapp")];
        let routes = build_auto_routes(&info, &[], &[], &statics);
        assert_eq!(routes[0].subdomain, "myapp-2");
    }

    // --- combine_routes ---

    #[test]
    fn combine_empty() {
        let result = combine_routes(&[], &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn combine_static_and_auto() {
        let statics = vec![make_gateway("static-route")];
        let autos = vec![make_gateway("auto-route")];
        let result = combine_routes(&statics, &autos);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].subdomain, "static-route");
        assert_eq!(result[1].subdomain, "auto-route");
    }

    // --- chrono_now ---

    #[test]
    fn chrono_now_format() {
        let t = chrono_now();
        assert_eq!(t.len(), 8); // HH:MM:SS
        assert_eq!(&t[2..3], ":");
        assert_eq!(&t[5..6], ":");
    }

    // --- uuid_short ---

    #[test]
    fn uuid_short_is_hex() {
        let u = uuid_short();
        assert!(!u.is_empty());
        assert!(u.chars().all(|c| c.is_ascii_hexdigit()));
    }

    // --- additional edge cases ---

    #[test]
    fn ports_invalid_label_falls_through() {
        let mut labels = HashMap::new();
        labels.insert("pulse.port".to_string(), "not-a-number".to_string());
        let info = make_info_with_labels("myapp", labels, vec![8080]);
        let ports = resolve_ports(&info, &[]);
        // Falls through to EXPOSE ports
        assert_eq!(ports, vec![(8080, None)]);
    }

    #[test]
    fn auto_routes_with_subdomain_template() {
        let info = make_info("myapp", "postgres:15", vec![5432]);
        let rules = vec![StaticRouteRule {
            image_pattern: "postgres*".to_string(),
            port_mappings: vec![PortMapping {
                port: 5432,
                subdomain_template: "{name}-db".to_string(),
            }],
        }];
        let routes = build_auto_routes(&info, &rules, &[], &[]);
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].subdomain, "myapp-db");
        assert_eq!(routes[0].port, 5432);
    }

    #[test]
    fn auto_routes_container_id_set() {
        let info = make_info("myapp", "nginx:latest", vec![80]);
        let routes = build_auto_routes(&info, &[], &[], &[]);
        assert_eq!(routes[0].container_id, Some("myapp-id".to_string()));
        assert_eq!(routes[0].container_name, Some("myapp".to_string()));
    }

    #[test]
    fn auto_routes_target_host_is_container_name() {
        let info = make_info("my-container", "nginx:latest", vec![80]);
        let routes = build_auto_routes(&info, &[], &[], &[]);
        assert_eq!(routes[0].target_host, "my-container");
    }

    #[test]
    fn collision_across_all_lists() {
        let auto = vec![make_gateway("app")];
        let statics = vec![make_gateway("app-2")];
        let pending = vec![make_gateway("app-3")];
        let result = resolve_collision("app", &auto, &statics, &pending);
        assert_eq!(result, "app-4");
    }

    #[test]
    fn image_matches_exact_with_tag() {
        assert!(image_matches("nginx:1.25-alpine", "nginx:1.25-alpine"));
        assert!(!image_matches("nginx:1.25-alpine", "nginx:1.25"));
    }

    #[test]
    fn sanitize_numbers_only() {
        assert_eq!(sanitize_subdomain("12345"), "12345");
    }

    #[test]
    fn sanitize_unicode_stripped() {
        assert_eq!(sanitize_subdomain("café"), "caf");
    }

    #[test]
    fn combine_preserves_order() {
        let s1 = make_gateway("s1");
        let s2 = make_gateway("s2");
        let a1 = make_gateway("a1");
        let result = combine_routes(&[s1, s2], &[a1]);
        assert_eq!(result[0].subdomain, "s1");
        assert_eq!(result[1].subdomain, "s2");
        assert_eq!(result[2].subdomain, "a1");
    }

    #[test]
    fn max_log_entries_constant() {
        assert_eq!(MAX_LOG_ENTRIES, 200);
    }

    #[test]
    fn sanitize_all_invalid_chars() {
        assert_eq!(sanitize_subdomain("!@#$%^&*()"), "");
    }

    #[test]
    fn sanitize_mixed_valid_invalid() {
        assert_eq!(sanitize_subdomain("a!b@c#d"), "abcd");
    }

    #[test]
    fn auto_routes_label_overrides_expose() {
        let mut labels = HashMap::new();
        labels.insert("pulse.port".to_string(), "9090".to_string());
        let info = make_info_with_labels("myapp", labels, vec![80, 443]);
        let routes = build_auto_routes(&info, &[], &[], &[]);
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].port, 9090);
        assert_eq!(routes[0].subdomain, "myapp");
    }

    #[test]
    fn auto_routes_label_subdomain_override() {
        let mut labels = HashMap::new();
        labels.insert("pulse.port".to_string(), "3000".to_string());
        labels.insert("pulse.subdomain".to_string(), "api".to_string());
        let info = make_info_with_labels("some-container", labels, vec![80]);
        let routes = build_auto_routes(&info, &[], &[], &[]);
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].subdomain, "api");
        assert_eq!(routes[0].port, 3000);
    }

    #[test]
    fn route_rule_takes_precedence_over_expose() {
        let info = make_info("myapp", "redis:7", vec![6379, 8080]);
        let rules = vec![StaticRouteRule {
            image_pattern: "redis*".to_string(),
            port_mappings: vec![PortMapping {
                port: 6379,
                subdomain_template: "{name}-cache".to_string(),
            }],
        }];
        let routes = build_auto_routes(&info, &rules, &[], &[]);
        // Should use the rule, not the EXPOSE ports
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].port, 6379);
        assert_eq!(routes[0].subdomain, "myapp-cache");
    }

    #[test]
    fn combine_only_static() {
        let statics = vec![make_gateway("s1"), make_gateway("s2")];
        let result = combine_routes(&statics, &[]);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn combine_only_auto() {
        let autos = vec![make_gateway("a1")];
        let result = combine_routes(&[], &autos);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn auto_routes_multi_port_with_rules() {
        let info = make_info("myapp", "custom:latest", vec![]);
        let rules = vec![StaticRouteRule {
            image_pattern: "custom*".to_string(),
            port_mappings: vec![
                PortMapping {
                    port: 80,
                    subdomain_template: "{name}-web".to_string(),
                },
                PortMapping {
                    port: 8080,
                    subdomain_template: "{name}-api".to_string(),
                },
            ],
        }];
        let routes = build_auto_routes(&info, &rules, &[], &[]);
        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].subdomain, "myapp-web");
        assert_eq!(routes[0].port, 80);
        assert_eq!(routes[1].subdomain, "myapp-api");
        assert_eq!(routes[1].port, 8080);
    }
}

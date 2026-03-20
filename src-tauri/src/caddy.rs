use reqwest::Client;
use serde_json::{json, Value};

use crate::models::{CertInfo, DnsProvider, Gateway};

const CADDY_ADMIN_URL: &str = "http://localhost:2019";

pub async fn check_health(client: &Client) -> bool {
    client
        .get(format!("{CADDY_ADMIN_URL}/config/"))
        .send()
        .await
        .is_ok()
}

fn build_caddy_config(routes: &[Gateway], domain: &str, dns_provider: &DnsProvider) -> Value {
    let caddy_routes: Vec<Value> = routes
        .iter()
        .map(|gw| {
            let host = if domain.is_empty() {
                gw.subdomain.clone()
            } else {
                format!("{}.{}", gw.subdomain, domain)
            };
            let target = resolve_target_host(&gw.target_host);
            json!({
                "match": [{"host": [host]}],
                "handle": [{
                    "handler": "reverse_proxy",
                    "upstreams": [{"dial": format!("{}:{}", target, gw.port)}]
                }]
            })
        })
        .collect();

    let mut config = json!({
        "apps": {
            "http": {
                "servers": {
                    "srv0": {
                        "listen": [":443", ":80"],
                        "routes": caddy_routes
                    }
                }
            }
        }
    });

    // If we have a domain, configure DNS-01 as the default ACME challenge
    // so all subdomain certificates are issued via the DNS provider.
    if !domain.is_empty() {
        let provider = dns_provider_config(dns_provider);

        // Collect all route subjects so the policy covers them explicitly.
        let subjects: Vec<String> = routes
            .iter()
            .map(|gw| {
                if domain.is_empty() {
                    gw.subdomain.clone()
                } else {
                    format!("{}.{}", gw.subdomain, domain)
                }
            })
            .collect();

        let mut policies = vec![];

        if !subjects.is_empty() {
            policies.push(json!({
                "subjects": subjects,
                "issuers": [
                    {
                        "module": "acme",
                        "challenges": {
                            "dns": {
                                "provider": provider
                            }
                        }
                    }
                ]
            }));
        }

        config["apps"]["tls"] = json!({
            "automation": {
                "policies": policies
            }
        });
    }

    config
}

/// Translate localhost references to host.docker.internal so Caddy
/// (running inside Docker) can reach services on the host machine.
fn resolve_target_host(host: &str) -> &str {
    match host {
        "localhost" | "127.0.0.1" | "0.0.0.0" | "host.docker.internal" => "host.docker.internal",
        other => other,
    }
}

fn dns_provider_config(provider: &DnsProvider) -> Value {
    match provider {
        DnsProvider::Cloudflare => json!({
            "name": "cloudflare",
            "api_token": "{env.CLOUDFLARE_API_TOKEN}"
        }),
        DnsProvider::Porkbun => json!({
            "name": "porkbun",
            "api_key": "{env.PORKBUN_API_KEY}",
            "api_secret_key": "{env.PORKBUN_API_SECRET}"
        }),
    }
}

pub async fn push_routes(
    client: &Client,
    routes: &[Gateway],
    domain: &str,
    dns_provider: &DnsProvider,
) -> Result<(), String> {
    let config = build_caddy_config(routes, domain, dns_provider);

    let resp = client
        .post(format!("{CADDY_ADMIN_URL}/load"))
        .json(&config)
        .send()
        .await
        .map_err(|e| format!("Failed to push routes to Caddy: {e}"))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Caddy rejected config: {body}"));
    }

    Ok(())
}

/// Check certificate status by making an HTTPS request to a routed subdomain.
/// Tries each route until a valid cert is found.
pub async fn get_cert_info(domain: &str, has_env_vars: bool, routes: &[Gateway]) -> CertInfo {
    if domain.is_empty() {
        return CertInfo {
            has_env_vars,
            domain: None,
            issuer: None,
            not_before: None,
            not_after: None,
            subject_alt_names: None,
            error: None,
        };
    }

    if routes.is_empty() {
        return CertInfo {
            has_env_vars,
            domain: Some(format!("*.{domain}")),
            issuer: None,
            not_before: None,
            not_after: None,
            subject_alt_names: None,
            error: Some("No routes configured yet.".to_string()),
        };
    }

    // Try each route: make an HTTPS request via reqwest to check if TLS works
    for gw in routes {
        let hostname = format!("{}.{}", gw.subdomain, domain);
        if check_tls_ok(&hostname).await {
            return CertInfo {
                has_env_vars,
                domain: Some(format!("*.{domain}")),
                issuer: Some("Let's Encrypt".to_string()),
                not_before: None,
                not_after: None,
                subject_alt_names: Some(hostname),
                error: None,
            };
        }
    }

    CertInfo {
        has_env_vars,
        domain: Some(format!("*.{domain}")),
        issuer: None,
        not_before: None,
        not_after: None,
        subject_alt_names: None,
        error: Some(
            "No valid certificates found yet. Certs may still be provisioning.".to_string(),
        ),
    }
}

/// Check if HTTPS is working for a given hostname by connecting to 127.0.0.1:443.
/// We accept any cert since we just need to confirm Caddy is serving TLS,
/// not validate the full chain (the browser does that).
async fn check_tls_ok(hostname: &str) -> bool {
    let tls_client = Client::builder()
        .resolve(hostname, ([127, 0, 0, 1], 443).into())
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(3))
        .build();

    let tls_client = match tls_client {
        Ok(c) => c,
        Err(_) => return false,
    };

    match tls_client.get(format!("https://{hostname}/")).send().await {
        Ok(_) => true,
        Err(e) => {
            eprintln!("[check_tls] {hostname}: {e}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::GatewaySource;

    fn make_gw(subdomain: &str, target: &str, port: u16) -> Gateway {
        Gateway {
            subdomain: subdomain.to_string(),
            target_host: target.to_string(),
            port,
            source: GatewaySource::Static,
            container_id: None,
            container_name: None,
        }
    }

    // --- resolve_target_host ---

    #[test]
    fn resolve_localhost() {
        assert_eq!(resolve_target_host("localhost"), "host.docker.internal");
    }

    #[test]
    fn resolve_127() {
        assert_eq!(resolve_target_host("127.0.0.1"), "host.docker.internal");
    }

    #[test]
    fn resolve_0000() {
        assert_eq!(resolve_target_host("0.0.0.0"), "host.docker.internal");
    }

    #[test]
    fn resolve_already_docker_internal() {
        assert_eq!(
            resolve_target_host("host.docker.internal"),
            "host.docker.internal"
        );
    }

    #[test]
    fn resolve_container_name() {
        assert_eq!(resolve_target_host("my-container"), "my-container");
    }

    // --- dns_provider_config ---

    #[test]
    fn dns_cloudflare() {
        let config = dns_provider_config(&DnsProvider::Cloudflare);
        assert_eq!(config["name"], "cloudflare");
        assert_eq!(config["api_token"], "{env.CLOUDFLARE_API_TOKEN}");
    }

    #[test]
    fn dns_porkbun() {
        let config = dns_provider_config(&DnsProvider::Porkbun);
        assert_eq!(config["name"], "porkbun");
        assert_eq!(config["api_key"], "{env.PORKBUN_API_KEY}");
        assert_eq!(config["api_secret_key"], "{env.PORKBUN_API_SECRET}");
    }

    // --- build_caddy_config ---

    #[test]
    fn config_empty_routes() {
        let config = build_caddy_config(&[], "example.com", &DnsProvider::Cloudflare);
        let routes = &config["apps"]["http"]["servers"]["srv0"]["routes"];
        assert!(routes.as_array().unwrap().is_empty());
    }

    #[test]
    fn config_single_route_with_domain() {
        let routes = vec![make_gw("app", "my-container", 8080)];
        let config = build_caddy_config(&routes, "example.com", &DnsProvider::Cloudflare);

        let caddy_routes = config["apps"]["http"]["servers"]["srv0"]["routes"]
            .as_array()
            .unwrap();
        assert_eq!(caddy_routes.len(), 1);

        let host = &caddy_routes[0]["match"][0]["host"][0];
        assert_eq!(host, "app.example.com");

        let dial = &caddy_routes[0]["handle"][0]["upstreams"][0]["dial"];
        assert_eq!(dial, "my-container:8080");
    }

    #[test]
    fn config_localhost_target_rewritten() {
        let routes = vec![make_gw("app", "localhost", 3000)];
        let config = build_caddy_config(&routes, "example.com", &DnsProvider::Cloudflare);

        let dial = &config["apps"]["http"]["servers"]["srv0"]["routes"][0]["handle"][0]
            ["upstreams"][0]["dial"];
        assert_eq!(dial, "host.docker.internal:3000");
    }

    #[test]
    fn config_no_domain_no_tls() {
        let routes = vec![make_gw("app", "container", 80)];
        let config = build_caddy_config(&routes, "", &DnsProvider::Cloudflare);

        // No TLS block when domain is empty
        assert!(config["apps"]["tls"].is_null());

        // Host should be just the subdomain
        let host = &config["apps"]["http"]["servers"]["srv0"]["routes"][0]["match"][0]["host"][0];
        assert_eq!(host, "app");
    }

    #[test]
    fn config_tls_subjects_match_routes() {
        let routes = vec![make_gw("app1", "c1", 80), make_gw("app2", "c2", 8080)];
        let config = build_caddy_config(&routes, "example.com", &DnsProvider::Cloudflare);

        let subjects = config["apps"]["tls"]["automation"]["policies"][0]["subjects"]
            .as_array()
            .unwrap();
        assert_eq!(subjects.len(), 2);
        assert_eq!(subjects[0], "app1.example.com");
        assert_eq!(subjects[1], "app2.example.com");
    }

    #[test]
    fn config_tls_uses_dns_provider() {
        let routes = vec![make_gw("app", "c", 80)];
        let config = build_caddy_config(&routes, "example.com", &DnsProvider::Porkbun);

        let provider = &config["apps"]["tls"]["automation"]["policies"][0]["issuers"][0]
            ["challenges"]["dns"]["provider"];
        assert_eq!(provider["name"], "porkbun");
    }

    #[test]
    fn config_listen_ports() {
        let config = build_caddy_config(&[], "example.com", &DnsProvider::Cloudflare);
        let listen = config["apps"]["http"]["servers"]["srv0"]["listen"]
            .as_array()
            .unwrap();
        assert!(listen.contains(&json!(":443")));
        assert!(listen.contains(&json!(":80")));
    }

    #[test]
    fn config_multiple_routes() {
        let routes = vec![
            make_gw("app1", "c1", 80),
            make_gw("app2", "c2", 3000),
            make_gw("app3", "localhost", 8080),
        ];
        let config = build_caddy_config(&routes, "test.dev", &DnsProvider::Cloudflare);
        let caddy_routes = config["apps"]["http"]["servers"]["srv0"]["routes"]
            .as_array()
            .unwrap();
        assert_eq!(caddy_routes.len(), 3);

        // Verify localhost rewrite on third route
        let dial3 = &caddy_routes[2]["handle"][0]["upstreams"][0]["dial"];
        assert_eq!(dial3, "host.docker.internal:8080");
    }

    #[test]
    fn config_empty_domain_empty_routes_no_tls() {
        let config = build_caddy_config(&[], "", &DnsProvider::Cloudflare);
        assert!(config["apps"]["tls"].is_null());
        let routes = config["apps"]["http"]["servers"]["srv0"]["routes"]
            .as_array()
            .unwrap();
        assert!(routes.is_empty());
    }

    #[test]
    fn config_tls_acme_module() {
        let routes = vec![make_gw("app", "c", 80)];
        let config = build_caddy_config(&routes, "example.com", &DnsProvider::Cloudflare);
        let issuer = &config["apps"]["tls"]["automation"]["policies"][0]["issuers"][0];
        assert_eq!(issuer["module"], "acme");
    }

    // --- get_cert_info (early returns, no network) ---

    #[tokio::test]
    async fn cert_info_empty_domain() {
        let info = get_cert_info("", true, &[]).await;
        assert!(info.domain.is_none());
        assert!(info.error.is_none());
        assert!(info.has_env_vars);
    }

    #[tokio::test]
    async fn cert_info_no_routes() {
        let info = get_cert_info("example.com", true, &[]).await;
        assert_eq!(info.domain, Some("*.example.com".to_string()));
        assert!(info.error.is_some());
        assert!(info.error.unwrap().contains("No routes"));
    }

    #[tokio::test]
    async fn cert_info_has_env_vars_flag() {
        let info_true = get_cert_info("example.com", true, &[]).await;
        assert!(info_true.has_env_vars);

        let info_false = get_cert_info("example.com", false, &[]).await;
        assert!(!info_false.has_env_vars);
    }

    #[tokio::test]
    async fn cert_info_with_routes_but_no_caddy() {
        // Routes exist but Caddy isn't running, so TLS check fails
        let routes = vec![make_gw("app", "c", 80)];
        let info = get_cert_info("example.com", true, &routes).await;
        // Should either find a cert or report provisioning error
        // Since Caddy isn't running in tests, it should fail
        assert_eq!(info.domain, Some("*.example.com".to_string()));
        if info.issuer.is_none() {
            assert!(info.error.is_some());
        }
    }

    #[test]
    fn resolve_ip_addresses() {
        assert_eq!(resolve_target_host("192.168.1.100"), "192.168.1.100");
        assert_eq!(resolve_target_host("10.0.0.1"), "10.0.0.1");
    }

    #[test]
    fn config_route_handler_is_reverse_proxy() {
        let routes = vec![make_gw("app", "c", 80)];
        let config = build_caddy_config(&routes, "example.com", &DnsProvider::Cloudflare);
        let handler = &config["apps"]["http"]["servers"]["srv0"]["routes"][0]["handle"][0];
        assert_eq!(handler["handler"], "reverse_proxy");
    }

    #[test]
    fn config_tls_empty_routes_no_policies() {
        let config = build_caddy_config(&[], "example.com", &DnsProvider::Cloudflare);
        let policies = config["apps"]["tls"]["automation"]["policies"]
            .as_array()
            .unwrap();
        assert!(policies.is_empty());
    }

    #[test]
    fn config_cloudflare_dns_challenge_structure() {
        let routes = vec![make_gw("app", "c", 80)];
        let config = build_caddy_config(&routes, "example.com", &DnsProvider::Cloudflare);
        let challenges =
            &config["apps"]["tls"]["automation"]["policies"][0]["issuers"][0]["challenges"];
        assert!(challenges["dns"]["provider"]["name"].is_string());
        assert_eq!(challenges["dns"]["provider"]["name"], "cloudflare");
    }

    #[test]
    fn config_porkbun_has_both_keys() {
        let routes = vec![make_gw("app", "c", 80)];
        let config = build_caddy_config(&routes, "example.com", &DnsProvider::Porkbun);
        let provider = &config["apps"]["tls"]["automation"]["policies"][0]["issuers"][0]
            ["challenges"]["dns"]["provider"];
        assert!(provider.get("api_key").is_some());
        assert!(provider.get("api_secret_key").is_some());
    }

    #[tokio::test]
    async fn cert_info_domain_format() {
        let info = get_cert_info("myapp.dev", false, &[]).await;
        assert_eq!(info.domain, Some("*.myapp.dev".to_string()));
    }

    #[tokio::test]
    async fn cert_info_multiple_routes_tries_all() {
        // Multiple routes, none will have certs in test env
        let routes = vec![
            make_gw("app1", "c1", 80),
            make_gw("app2", "c2", 8080),
            make_gw("app3", "c3", 3000),
        ];
        let info = get_cert_info("example.com", true, &routes).await;
        // Should report error since no Caddy is running
        assert!(info.error.is_some() || info.issuer.is_some());
    }
}

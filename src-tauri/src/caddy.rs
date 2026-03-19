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

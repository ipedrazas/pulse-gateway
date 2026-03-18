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

    // If we have a domain, add a wildcard TLS automation policy so Caddy
    // requests a single *.domain certificate via DNS-01 instead of
    // per-route certificates via HTTP-01.
    if !domain.is_empty() {
        let wildcard = format!("*.{domain}");
        let provider = dns_provider_config(dns_provider);
        config["apps"]["tls"] = json!({
            "automation": {
                "policies": [
                    {
                        "subjects": [wildcard],
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
                    }
                ]
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

/// Check certificate status by probing localhost:443 with openssl.
pub fn get_cert_info(domain: &str, has_env_vars: bool) -> CertInfo {
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

    match get_cert_details_openssl(domain) {
        Some(details) => CertInfo {
            has_env_vars,
            domain: Some(format!("*.{domain}")),
            issuer: details.issuer,
            not_before: details.not_before,
            not_after: details.not_after,
            subject_alt_names: details.sans,
            error: None,
        },
        None => CertInfo {
            has_env_vars,
            domain: Some(format!("*.{domain}")),
            issuer: None,
            not_before: None,
            not_after: None,
            subject_alt_names: None,
            error: Some("Could not retrieve certificate. Is Caddy running with HTTPS?".to_string()),
        },
    }
}

struct CertDetails {
    issuer: Option<String>,
    not_before: Option<String>,
    not_after: Option<String>,
    sans: Option<String>,
}

fn get_cert_details_openssl(domain: &str) -> Option<CertDetails> {
    use std::process::Command;

    let hostname = format!("test.{domain}");
    let output = Command::new("sh")
        .args([
            "-c",
            &format!(
                "echo | openssl s_client -connect 127.0.0.1:443 -servername {} 2>/dev/null | openssl x509 -noout -issuer -startdate -enddate -ext subjectAltName 2>/dev/null",
                hostname
            ),
        ])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        return None;
    }

    let mut issuer = None;
    let mut not_before = None;
    let mut not_after = None;
    let mut sans = None;

    for line in stdout.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("issuer=") {
            // Extract CN from issuer string like "/C=US/O=Let's Encrypt/CN=R11"
            issuer = Some(
                val.split("CN = ").nth(1)
                    .or_else(|| val.split("CN=").nth(1))
                    .unwrap_or(val)
                    .to_string()
            );
        } else if let Some(val) = line.strip_prefix("notBefore=") {
            not_before = Some(val.to_string());
        } else if let Some(val) = line.strip_prefix("notAfter=") {
            not_after = Some(val.to_string());
        } else if line.starts_with("DNS:") {
            sans = Some(
                line.split(", ")
                    .map(|s| s.trim_start_matches("DNS:"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }

    Some(CertDetails {
        issuer,
        not_before,
        not_after,
        sans,
    })
}

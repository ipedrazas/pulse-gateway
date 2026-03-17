use reqwest::Client;
use serde_json::{json, Value};

use crate::credentials;
use crate::models::{CertInfo, DnsProvider, Gateway};

const CADDY_ADMIN_URL: &str = "http://localhost:2019";

pub async fn check_health(client: &Client) -> bool {
    client
        .get(format!("{CADDY_ADMIN_URL}/config/"))
        .send()
        .await
        .is_ok()
}

fn build_tls_automation(domain: &str, provider: &DnsProvider) -> Option<Value> {
    if *provider == DnsProvider::None || domain.is_empty() {
        return None;
    }

    let dns_provider = match provider {
        DnsProvider::Cloudflare => {
            let token = credentials::get_cloudflare_token().ok()?;
            json!({
                "name": "cloudflare",
                "api_token": token
            })
        }
        DnsProvider::Porkbun => {
            let (key, secret) = credentials::get_porkbun_keys().ok()?;
            json!({
                "name": "porkbun",
                "api_key": key,
                "api_secret_key": secret
            })
        }
        DnsProvider::None => return None,
    };

    Some(json!({
        "automation": {
            "policies": [{
                "subjects": [format!("*.{domain}")],
                "issuers": [{
                    "module": "acme",
                    "challenges": {
                        "dns": {
                            "provider": dns_provider
                        }
                    }
                }]
            }]
        }
    }))
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
            json!({
                "match": [{"host": [host]}],
                "handle": [{
                    "handler": "reverse_proxy",
                    "upstreams": [{"dial": format!("{}:{}", gw.target_host, gw.port)}]
                }]
            })
        })
        .collect();

    let mut apps = json!({
        "http": {
            "servers": {
                "srv0": {
                    "listen": [":443", ":80"],
                    "routes": caddy_routes
                }
            }
        }
    });

    // Add TLS automation if DNS provider is configured
    if let Some(tls_config) = build_tls_automation(domain, dns_provider) {
        apps.as_object_mut()
            .unwrap()
            .insert("tls".to_string(), tls_config);
    }

    json!({ "apps": apps })
}

pub async fn push_routes_with_tls(
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

/// Get certificate information by checking the Caddy TLS config
/// and attempting to read cert expiry via openssl.
pub async fn get_cert_info(client: &Client, domain: &str, provider: &DnsProvider) -> CertInfo {
    if *provider == DnsProvider::None || domain.is_empty() {
        return CertInfo {
            configured: false,
            domain: None,
            expiry: None,
            error: None,
        };
    }

    // Check if TLS is configured in Caddy
    let tls_configured = match client
        .get(format!("{CADDY_ADMIN_URL}/config/apps/tls/"))
        .send()
        .await
    {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    };

    if !tls_configured {
        return CertInfo {
            configured: false,
            domain: Some(format!("*.{domain}")),
            expiry: None,
            error: Some("TLS automation not yet pushed to Caddy".to_string()),
        };
    }

    // Try to get cert expiry via openssl
    let expiry = get_cert_expiry_openssl(domain);

    CertInfo {
        configured: true,
        domain: Some(format!("*.{domain}")),
        expiry,
        error: None,
    }
}

/// Use openssl to check certificate expiration on localhost:443.
fn get_cert_expiry_openssl(domain: &str) -> Option<String> {
    use std::process::Command;

    let hostname = format!("test.{domain}");
    let output = Command::new("sh")
        .args([
            "-c",
            &format!(
                "echo | openssl s_client -connect 127.0.0.1:443 -servername {} 2>/dev/null | openssl x509 -noout -enddate 2>/dev/null",
                hostname
            ),
        ])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Output is like: notAfter=Mar 17 00:00:00 2026 GMT
    stdout
        .trim()
        .strip_prefix("notAfter=")
        .map(|s| s.to_string())
}

use reqwest::Client;
use serde_json::{json, Value};

use crate::models::{CertInfo, Gateway};

const CADDY_ADMIN_URL: &str = "http://localhost:2019";

pub async fn check_health(client: &Client) -> bool {
    client
        .get(format!("{CADDY_ADMIN_URL}/config/"))
        .send()
        .await
        .is_ok()
}

fn build_caddy_config(routes: &[Gateway], domain: &str) -> Value {
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

    json!({
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
    })
}

pub async fn push_routes(
    client: &Client,
    routes: &[Gateway],
    domain: &str,
) -> Result<(), String> {
    let config = build_caddy_config(routes, domain);

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
            expiry: None,
            error: None,
        };
    }

    let expiry = get_cert_expiry_openssl(domain);

    CertInfo {
        has_env_vars,
        domain: Some(format!("*.{domain}")),
        expiry,
        error: None,
    }
}

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
    stdout
        .trim()
        .strip_prefix("notAfter=")
        .map(|s| s.to_string())
}

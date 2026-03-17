use keyring::Entry;

use crate::models::DnsProvider;

const SERVICE: &str = "dev.andcake.pulsegw";

fn entry(username: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, username).map_err(|e| format!("Keyring error: {e}"))
}

pub fn store_cloudflare_token(token: &str) -> Result<(), String> {
    entry("cloudflare_api_token")?
        .set_password(token)
        .map_err(|e| format!("Failed to store Cloudflare token: {e}"))
}

pub fn get_cloudflare_token() -> Result<String, String> {
    entry("cloudflare_api_token")?
        .get_password()
        .map_err(|e| format!("Failed to get Cloudflare token: {e}"))
}

pub fn store_porkbun_keys(api_key: &str, api_secret: &str) -> Result<(), String> {
    entry("porkbun_api_key")?
        .set_password(api_key)
        .map_err(|e| format!("Failed to store Porkbun API key: {e}"))
        ?;
    entry("porkbun_api_secret")?
        .set_password(api_secret)
        .map_err(|e| format!("Failed to store Porkbun API secret: {e}"))
}

pub fn get_porkbun_keys() -> Result<(String, String), String> {
    let key = entry("porkbun_api_key")?
        .get_password()
        .map_err(|e| format!("Failed to get Porkbun API key: {e}"))?;
    let secret = entry("porkbun_api_secret")?
        .get_password()
        .map_err(|e| format!("Failed to get Porkbun API secret: {e}"))?;
    Ok((key, secret))
}

pub fn has_credentials(provider: &DnsProvider) -> bool {
    match provider {
        DnsProvider::None => false,
        DnsProvider::Cloudflare => get_cloudflare_token().is_ok(),
        DnsProvider::Porkbun => get_porkbun_keys().is_ok(),
    }
}

pub fn delete_credentials(provider: &DnsProvider) {
    match provider {
        DnsProvider::None => {}
        DnsProvider::Cloudflare => {
            let _ = entry("cloudflare_api_token").and_then(|e| {
                e.delete_credential().map_err(|e| e.to_string())
            });
        }
        DnsProvider::Porkbun => {
            let _ = entry("porkbun_api_key").and_then(|e| {
                e.delete_credential().map_err(|e| e.to_string())
            });
            let _ = entry("porkbun_api_secret").and_then(|e| {
                e.delete_credential().map_err(|e| e.to_string())
            });
        }
    }
}

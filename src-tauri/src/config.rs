use std::fs;
use std::path::PathBuf;
use tauri::Manager;

use crate::models::AppConfig;

fn config_path(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config dir: {e}"))?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create config dir: {e}"))?;
    Ok(dir.join("config.json"))
}

pub fn load_config(app_handle: &tauri::AppHandle) -> AppConfig {
    match config_path(app_handle) {
        Ok(path) => match fs::read_to_string(&path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
            Err(_) => AppConfig::default(),
        },
        Err(_) => AppConfig::default(),
    }
}

pub fn save_config(app_handle: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    let path = config_path(app_handle)?;
    save_config_to_path(&path, config)
}

/// Save config to a specific path (extracted for testability).
fn save_config_to_path(path: &PathBuf, config: &AppConfig) -> Result<(), String> {
    let data = serde_json::to_string_pretty(config).map_err(|e| format!("Serialize error: {e}"))?;
    fs::write(path, data).map_err(|e| format!("Write error: {e}"))?;
    Ok(())
}

/// Load config from a specific path (extracted for testability).
#[cfg(test)]
fn load_config_from_path(path: &PathBuf) -> AppConfig {
    match fs::read_to_string(path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DnsProvider, EnvVarEntry, Gateway, GatewaySource};
    use tempfile::TempDir;

    #[test]
    fn load_missing_file_returns_default() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.json");
        let config = load_config_from_path(&path);
        assert_eq!(config.domain, "");
        assert_eq!(config.caddy_image, "caddy:2");
        assert_eq!(config.dns_provider, DnsProvider::Cloudflare);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");

        let config = AppConfig {
            domain: "test.dev".to_string(),
            caddy_image: "caddy:custom".to_string(),
            dns_provider: DnsProvider::Porkbun,
            static_routes: vec![Gateway {
                subdomain: "app".to_string(),
                target_host: "localhost".to_string(),
                port: 3000,
                source: GatewaySource::Static,
                container_id: None,
                container_name: None,
            }],
            route_rules: vec![],
            caddy_env_vars: vec![EnvVarEntry {
                key: "TOKEN".to_string(),
            }],
        };

        save_config_to_path(&path, &config).unwrap();
        let loaded = load_config_from_path(&path);

        assert_eq!(loaded.domain, "test.dev");
        assert_eq!(loaded.caddy_image, "caddy:custom");
        assert_eq!(loaded.dns_provider, DnsProvider::Porkbun);
        assert_eq!(loaded.static_routes.len(), 1);
        assert_eq!(loaded.caddy_env_vars.len(), 1);
    }

    #[test]
    fn load_invalid_json_returns_default() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, "not valid json!!!").unwrap();

        let config = load_config_from_path(&path);
        assert_eq!(config.domain, "");
        assert_eq!(config.caddy_image, "caddy:2");
    }

    #[test]
    fn load_partial_json_fills_defaults() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");
        fs::write(
            &path,
            r#"{"domain":"partial.dev","caddy_image":"caddy:2","static_routes":[]}"#,
        )
        .unwrap();

        let config = load_config_from_path(&path);
        assert_eq!(config.domain, "partial.dev");
        assert_eq!(config.dns_provider, DnsProvider::Cloudflare);
        assert!(config.route_rules.is_empty());
        assert!(config.caddy_env_vars.is_empty());
    }

    #[test]
    fn save_creates_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("new_config.json");
        assert!(!path.exists());

        save_config_to_path(&path, &AppConfig::default()).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn save_overwrites_existing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");

        let config1 = AppConfig {
            domain: "first.dev".to_string(),
            ..AppConfig::default()
        };
        save_config_to_path(&path, &config1).unwrap();

        let config2 = AppConfig {
            domain: "second.dev".to_string(),
            ..AppConfig::default()
        };
        save_config_to_path(&path, &config2).unwrap();

        let loaded = load_config_from_path(&path);
        assert_eq!(loaded.domain, "second.dev");
    }
}

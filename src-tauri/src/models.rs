use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum GatewaySource {
    #[default]
    Static,
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gateway {
    pub subdomain: String,
    pub target_host: String,
    pub port: u16,
    #[serde(default)]
    pub source: GatewaySource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaddyStatus {
    pub running: bool,
    pub api_reachable: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMapping {
    pub port: u16,
    pub subdomain_template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticRouteRule {
    pub image_pattern: String,
    pub port_mappings: Vec<PortMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertInfo {
    pub has_env_vars: bool,
    pub domain: Option<String>,
    pub expiry: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

/// An env var to pass to the Caddy container. The key is stored in config,
/// the value is stored in the system keyring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVarEntry {
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub domain: String,
    pub caddy_image: String,
    pub static_routes: Vec<Gateway>,
    #[serde(default)]
    pub route_rules: Vec<StaticRouteRule>,
    #[serde(default)]
    pub caddy_env_vars: Vec<EnvVarEntry>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            domain: String::new(),
            caddy_image: "caddy:2".to_string(),
            static_routes: Vec::new(),
            route_rules: Vec::new(),
            caddy_env_vars: Vec::new(),
        }
    }
}

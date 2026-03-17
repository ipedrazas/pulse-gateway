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

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DnsProvider {
    #[default]
    None,
    Cloudflare,
    Porkbun,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsConfig {
    pub provider: DnsProvider,
    /// Whether credentials are stored in the keyring (not the actual secrets).
    pub has_credentials: bool,
}

impl Default for DnsConfig {
    fn default() -> Self {
        Self {
            provider: DnsProvider::None,
            has_credentials: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertInfo {
    pub configured: bool,
    pub domain: Option<String>,
    pub expiry: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub domain: String,
    pub caddy_image: String,
    pub static_routes: Vec<Gateway>,
    #[serde(default)]
    pub route_rules: Vec<StaticRouteRule>,
    #[serde(default)]
    pub dns_provider: DnsProvider,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            domain: String::new(),
            caddy_image: "caddy:2".to_string(),
            static_routes: Vec::new(),
            route_rules: Vec::new(),
            dns_provider: DnsProvider::None,
        }
    }
}

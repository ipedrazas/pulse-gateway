use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gateway {
    pub subdomain: String,
    pub target_host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaddyStatus {
    pub running: bool,
    pub api_reachable: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub domain: String,
    pub caddy_image: String,
    pub static_routes: Vec<Gateway>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            domain: String::new(),
            caddy_image: "caddy:2".to_string(),
            static_routes: Vec::new(),
        }
    }
}

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
    pub issuer: Option<String>,
    pub not_before: Option<String>,
    pub not_after: Option<String>,
    pub subject_alt_names: Option<String>,
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

/// Supported DNS challenge providers for wildcard certificates.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DnsProvider {
    #[default]
    Cloudflare,
    Porkbun,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub domain: String,
    pub caddy_image: String,
    #[serde(default)]
    pub dns_provider: DnsProvider,
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
            dns_provider: DnsProvider::default(),
            static_routes: Vec::new(),
            route_rules: Vec::new(),
            caddy_env_vars: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_config_defaults() {
        let config = AppConfig::default();
        assert_eq!(config.domain, "");
        assert_eq!(config.caddy_image, "caddy:2");
        assert_eq!(config.dns_provider, DnsProvider::Cloudflare);
        assert!(config.static_routes.is_empty());
        assert!(config.route_rules.is_empty());
        assert!(config.caddy_env_vars.is_empty());
    }

    #[test]
    fn gateway_source_default() {
        assert_eq!(GatewaySource::default(), GatewaySource::Static);
    }

    #[test]
    fn dns_provider_default() {
        assert_eq!(DnsProvider::default(), DnsProvider::Cloudflare);
    }

    #[test]
    fn gateway_source_json_rename() {
        let json = serde_json::to_string(&GatewaySource::Static).unwrap();
        assert_eq!(json, r#""static""#);
        let json = serde_json::to_string(&GatewaySource::Auto).unwrap();
        assert_eq!(json, r#""auto""#);
    }

    #[test]
    fn dns_provider_json_rename() {
        let json = serde_json::to_string(&DnsProvider::Cloudflare).unwrap();
        assert_eq!(json, r#""cloudflare""#);
        let json = serde_json::to_string(&DnsProvider::Porkbun).unwrap();
        assert_eq!(json, r#""porkbun""#);
    }

    #[test]
    fn gateway_serde_roundtrip() {
        let gw = Gateway {
            subdomain: "myapp".to_string(),
            target_host: "container-1".to_string(),
            port: 8080,
            source: GatewaySource::Auto,
            container_id: Some("abc123".to_string()),
            container_name: Some("my-container".to_string()),
        };
        let json = serde_json::to_string(&gw).unwrap();
        let parsed: Gateway = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.subdomain, "myapp");
        assert_eq!(parsed.port, 8080);
        assert_eq!(parsed.source, GatewaySource::Auto);
        assert_eq!(parsed.container_id, Some("abc123".to_string()));
    }

    #[test]
    fn gateway_optional_fields_omitted() {
        let gw = Gateway {
            subdomain: "app".to_string(),
            target_host: "host".to_string(),
            port: 80,
            source: GatewaySource::Static,
            container_id: None,
            container_name: None,
        };
        let json = serde_json::to_string(&gw).unwrap();
        assert!(!json.contains("container_id"));
        assert!(!json.contains("container_name"));
    }

    #[test]
    fn app_config_serde_roundtrip() {
        let config = AppConfig {
            domain: "example.com".to_string(),
            caddy_image: "caddy:2".to_string(),
            dns_provider: DnsProvider::Porkbun,
            static_routes: vec![],
            route_rules: vec![],
            caddy_env_vars: vec![EnvVarEntry {
                key: "API_KEY".to_string(),
            }],
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.domain, "example.com");
        assert_eq!(parsed.dns_provider, DnsProvider::Porkbun);
        assert_eq!(parsed.caddy_env_vars.len(), 1);
        assert_eq!(parsed.caddy_env_vars[0].key, "API_KEY");
    }

    #[test]
    fn app_config_missing_optional_fields() {
        // dns_provider, route_rules, caddy_env_vars should default when missing
        let json = r#"{"domain":"test.com","caddy_image":"caddy:2","static_routes":[]}"#;
        let config: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.dns_provider, DnsProvider::Cloudflare);
        assert!(config.route_rules.is_empty());
        assert!(config.caddy_env_vars.is_empty());
    }

    // --- CaddyStatus ---

    #[test]
    fn caddy_status_serde() {
        let status = CaddyStatus {
            running: true,
            api_reachable: false,
            error: Some("test error".to_string()),
        };
        let json = serde_json::to_string(&status).unwrap();
        let parsed: CaddyStatus = serde_json::from_str(&json).unwrap();
        assert!(parsed.running);
        assert!(!parsed.api_reachable);
        assert_eq!(parsed.error, Some("test error".to_string()));
    }

    #[test]
    fn caddy_status_no_error() {
        let status = CaddyStatus {
            running: true,
            api_reachable: true,
            error: None,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("null"));
    }

    // --- CertInfo ---

    #[test]
    fn cert_info_serde() {
        let info = CertInfo {
            has_env_vars: true,
            domain: Some("*.example.com".to_string()),
            issuer: Some("Let's Encrypt".to_string()),
            not_before: None,
            not_after: Some("Jun 2026".to_string()),
            subject_alt_names: Some("app.example.com".to_string()),
            error: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: CertInfo = serde_json::from_str(&json).unwrap();
        assert!(parsed.has_env_vars);
        assert_eq!(parsed.issuer, Some("Let's Encrypt".to_string()));
        assert_eq!(parsed.not_after, Some("Jun 2026".to_string()));
    }

    #[test]
    fn cert_info_all_none() {
        let info = CertInfo {
            has_env_vars: false,
            domain: None,
            issuer: None,
            not_before: None,
            not_after: None,
            subject_alt_names: None,
            error: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: CertInfo = serde_json::from_str(&json).unwrap();
        assert!(!parsed.has_env_vars);
        assert!(parsed.domain.is_none());
    }

    // --- LogEntry ---

    #[test]
    fn log_entry_serde() {
        let entry = LogEntry {
            timestamp: "12:34:56".to_string(),
            level: "info".to_string(),
            message: "test message".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: LogEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.timestamp, "12:34:56");
        assert_eq!(parsed.level, "info");
        assert_eq!(parsed.message, "test message");
    }

    // --- PortMapping ---

    #[test]
    fn port_mapping_serde() {
        let pm = PortMapping {
            port: 8080,
            subdomain_template: "{name}-api".to_string(),
        };
        let json = serde_json::to_string(&pm).unwrap();
        let parsed: PortMapping = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.port, 8080);
        assert_eq!(parsed.subdomain_template, "{name}-api");
    }

    // --- StaticRouteRule ---

    #[test]
    fn static_route_rule_serde() {
        let rule = StaticRouteRule {
            image_pattern: "postgres*".to_string(),
            port_mappings: vec![PortMapping {
                port: 5432,
                subdomain_template: "{name}-db".to_string(),
            }],
        };
        let json = serde_json::to_string(&rule).unwrap();
        let parsed: StaticRouteRule = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.image_pattern, "postgres*");
        assert_eq!(parsed.port_mappings.len(), 1);
        assert_eq!(parsed.port_mappings[0].port, 5432);
    }

    // --- EnvVarEntry ---

    #[test]
    fn env_var_entry_serde() {
        let entry = EnvVarEntry {
            key: "CLOUDFLARE_API_TOKEN".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: EnvVarEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.key, "CLOUDFLARE_API_TOKEN");
    }

    // --- Gateway source default in deserialization ---

    #[test]
    fn gateway_source_defaults_to_static_when_missing() {
        let json = r#"{"subdomain":"app","target_host":"c","port":80}"#;
        let gw: Gateway = serde_json::from_str(json).unwrap();
        assert_eq!(gw.source, GatewaySource::Static);
    }

    // --- AppConfig with full data ---

    #[test]
    fn app_config_with_route_rules() {
        let config = AppConfig {
            domain: "test.dev".to_string(),
            caddy_image: "custom:latest".to_string(),
            dns_provider: DnsProvider::Porkbun,
            static_routes: vec![Gateway {
                subdomain: "app".to_string(),
                target_host: "localhost".to_string(),
                port: 3000,
                source: GatewaySource::Static,
                container_id: None,
                container_name: None,
            }],
            route_rules: vec![StaticRouteRule {
                image_pattern: "redis*".to_string(),
                port_mappings: vec![PortMapping {
                    port: 6379,
                    subdomain_template: "{name}-cache".to_string(),
                }],
            }],
            caddy_env_vars: vec![
                EnvVarEntry {
                    key: "KEY1".to_string(),
                },
                EnvVarEntry {
                    key: "KEY2".to_string(),
                },
            ],
        };
        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.static_routes.len(), 1);
        assert_eq!(parsed.route_rules.len(), 1);
        assert_eq!(parsed.caddy_env_vars.len(), 2);
        assert_eq!(parsed.dns_provider, DnsProvider::Porkbun);
    }
}

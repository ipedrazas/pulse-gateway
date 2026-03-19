use std::collections::HashMap;

use bollard::container::{
    Config, CreateContainerOptions, InspectContainerOptions, ListContainersOptions,
    StartContainerOptions,
};
use bollard::image::CreateImageOptions;
use bollard::models::{
    EndpointSettings, HostConfig, Mount, MountTypeEnum, PortBinding, RestartPolicy,
    RestartPolicyNameEnum,
};
use bollard::network::{ConnectNetworkOptions, CreateNetworkOptions};
use bollard::Docker;
use futures::StreamExt;

pub const NETWORK_NAME: &str = "pulse-gateway";
pub const CADDY_CONTAINER_NAME: &str = "pulse-caddy";

pub fn connect() -> Result<Docker, String> {
    Docker::connect_with_local_defaults().map_err(|e| format!("Docker connection failed: {e}"))
}

fn docker_err(context: &str, e: bollard::errors::Error) -> String {
    let msg = e.to_string();
    if msg.contains("client error (Connect)")
        || msg.contains("No such file or directory")
        || msg.contains("connection refused")
    {
        "Docker does not appear to be running. Please start Docker Desktop and try again."
            .to_string()
    } else {
        format!("{context}: {e}")
    }
}

pub async fn ensure_network(docker: &Docker) -> Result<(), String> {
    let networks = docker
        .list_networks::<String>(None)
        .await
        .map_err(|e| docker_err("Failed to list networks", e))?;

    let exists = networks
        .iter()
        .any(|n| n.name.as_deref() == Some(NETWORK_NAME));

    if !exists {
        docker
            .create_network(CreateNetworkOptions {
                name: NETWORK_NAME,
                driver: "bridge",
                ..Default::default()
            })
            .await
            .map_err(|e| docker_err("Failed to create network", e))?;
    }

    Ok(())
}

pub async fn ensure_caddy(
    docker: &Docker,
    image: &str,
    env_vars: &[(String, String)],
) -> Result<(), String> {
    let filters: HashMap<String, Vec<String>> =
        HashMap::from([("name".to_string(), vec![CADDY_CONTAINER_NAME.to_string()])]);
    let containers = docker
        .list_containers(Some(ListContainersOptions {
            all: true,
            filters,
            ..Default::default()
        }))
        .await
        .map_err(|e| docker_err("Failed to list containers", e))?;

    if let Some(container) = containers.first() {
        let needs_recreate = check_needs_recreate(docker, image, env_vars).await;

        if needs_recreate {
            let _ = docker.stop_container(CADDY_CONTAINER_NAME, None).await;
            let _ = docker.remove_container(CADDY_CONTAINER_NAME, None).await;
            // Fall through to create a new container
        } else {
            let state = container.state.as_deref().unwrap_or("");
            if state != "running" {
                docker
                    .start_container(CADDY_CONTAINER_NAME, None::<StartContainerOptions<String>>)
                    .await
                    .map_err(|e| docker_err("Failed to start Caddy", e))?;
            }
            return Ok(());
        }
    }

    // Pull the image
    let mut pull_stream = docker.create_image(
        Some(CreateImageOptions {
            from_image: image,
            ..Default::default()
        }),
        None,
        None,
    );
    while let Some(result) = pull_stream.next().await {
        result.map_err(|e| docker_err("Failed to pull image", e))?;
    }

    // Build env var list for the container.
    // Always set CADDY_ADMIN to bind on all interfaces so the host can reach it.
    let mut env: Vec<String> = vec!["CADDY_ADMIN=0.0.0.0:2019".to_string()];
    for (k, v) in env_vars {
        env.push(format!("{k}={v}"));
    }

    // Create and start the container
    let mut exposed_ports = HashMap::new();
    exposed_ports.insert("443/tcp".to_string(), HashMap::new());
    exposed_ports.insert("80/tcp".to_string(), HashMap::new());
    exposed_ports.insert("2019/tcp".to_string(), HashMap::new());

    let mut port_bindings = HashMap::new();
    port_bindings.insert(
        "443/tcp".to_string(),
        Some(vec![PortBinding {
            host_ip: Some("0.0.0.0".to_string()),
            host_port: Some("443".to_string()),
        }]),
    );
    port_bindings.insert(
        "80/tcp".to_string(),
        Some(vec![PortBinding {
            host_ip: Some("0.0.0.0".to_string()),
            host_port: Some("80".to_string()),
        }]),
    );
    port_bindings.insert(
        "2019/tcp".to_string(),
        Some(vec![PortBinding {
            host_ip: Some("127.0.0.1".to_string()),
            host_port: Some("2019".to_string()),
        }]),
    );

    let config = Config {
        image: Some(image.to_string()),
        env: Some(env),
        exposed_ports: Some(exposed_ports),
        host_config: Some(HostConfig {
            network_mode: Some(NETWORK_NAME.to_string()),
            port_bindings: Some(port_bindings),
            mounts: Some(vec![
                Mount {
                    target: Some("/config".to_string()),
                    source: Some("pulse-caddy-config".to_string()),
                    typ: Some(MountTypeEnum::VOLUME),
                    ..Default::default()
                },
                Mount {
                    target: Some("/data".to_string()),
                    source: Some("pulse-caddy-data".to_string()),
                    typ: Some(MountTypeEnum::VOLUME),
                    ..Default::default()
                },
            ]),
            restart_policy: Some(RestartPolicy {
                name: Some(RestartPolicyNameEnum::UNLESS_STOPPED),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    docker
        .create_container(
            Some(CreateContainerOptions {
                name: CADDY_CONTAINER_NAME,
                ..Default::default()
            }),
            config,
        )
        .await
        .map_err(|e| docker_err("Failed to create Caddy container", e))?;

    docker
        .start_container(CADDY_CONTAINER_NAME, None::<StartContainerOptions<String>>)
        .await
        .map_err(|e| docker_err("Failed to start Caddy", e))?;

    Ok(())
}

/// Check if the existing Caddy container needs to be recreated
/// (image or env vars changed).
async fn check_needs_recreate(
    docker: &Docker,
    expected_image: &str,
    expected_env: &[(String, String)],
) -> bool {
    let info = match docker
        .inspect_container(CADDY_CONTAINER_NAME, None::<InspectContainerOptions>)
        .await
    {
        Ok(info) => info,
        Err(_) => return true,
    };

    // Check image
    let current_image = info
        .config
        .as_ref()
        .and_then(|c| c.image.as_deref())
        .unwrap_or("");
    if current_image != expected_image {
        return true;
    }

    // Check env vars — extract current env as key=value pairs
    let current_env: Vec<&str> = info
        .config
        .as_ref()
        .and_then(|c| c.env.as_ref())
        .map(|e| e.iter().map(|s| s.as_str()).collect())
        .unwrap_or_default();

    // Check CADDY_ADMIN is set
    if !current_env.contains(&"CADDY_ADMIN=0.0.0.0:2019") {
        return true;
    }

    // Check that all expected env vars are present with correct values
    for (k, v) in expected_env {
        let expected = format!("{k}={v}");
        if !current_env.contains(&expected.as_str()) {
            return true;
        }
    }

    false
}

pub async fn stop_caddy(docker: &Docker) -> Result<(), String> {
    docker
        .stop_container(CADDY_CONTAINER_NAME, None)
        .await
        .map_err(|e| docker_err("Failed to stop Caddy", e))?;
    Ok(())
}

pub async fn is_caddy_running(docker: &Docker) -> bool {
    let filters: HashMap<String, Vec<String>> =
        HashMap::from([("name".to_string(), vec![CADDY_CONTAINER_NAME.to_string()])]);
    match docker
        .list_containers(Some(ListContainersOptions {
            all: false,
            filters,
            ..Default::default()
        }))
        .await
    {
        Ok(containers) => !containers.is_empty(),
        Err(_) => false,
    }
}

/// Information extracted from a running container for routing purposes.
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub labels: HashMap<String, String>,
    pub ports: Vec<u16>,
    pub on_network: bool,
}

pub async fn inspect_for_routing(
    docker: &Docker,
    container_id: &str,
) -> Result<ContainerInfo, String> {
    let info = docker
        .inspect_container(container_id, None::<InspectContainerOptions>)
        .await
        .map_err(|e| docker_err("Failed to inspect container", e))?;

    let name = info
        .name
        .unwrap_or_default()
        .trim_start_matches('/')
        .to_string();

    let config = info.config.unwrap_or_default();
    let labels = config.labels.unwrap_or_default();
    let image = config.image.unwrap_or_default();

    let exposed_ports = config.exposed_ports.unwrap_or_default();
    let ports: Vec<u16> = exposed_ports
        .keys()
        .filter_map(|k| k.split('/').next()?.parse().ok())
        .collect();

    let on_network = info
        .network_settings
        .as_ref()
        .and_then(|ns| ns.networks.as_ref())
        .map(|n| n.contains_key(NETWORK_NAME))
        .unwrap_or(false);

    Ok(ContainerInfo {
        id: container_id.to_string(),
        name,
        image,
        labels,
        ports,
        on_network,
    })
}

pub async fn attach_to_network(docker: &Docker, container_id: &str) -> Result<(), String> {
    docker
        .connect_network(
            NETWORK_NAME,
            ConnectNetworkOptions {
                container: container_id.to_string(),
                endpoint_config: EndpointSettings::default(),
            },
        )
        .await
        .map_err(|e| docker_err("Failed to attach container to network", e))?;
    Ok(())
}

pub async fn list_running_containers(docker: &Docker) -> Result<Vec<String>, String> {
    let containers = docker
        .list_containers(Some(ListContainersOptions::<String> {
            all: false,
            ..Default::default()
        }))
        .await
        .map_err(|e| docker_err("Failed to list containers", e))?;

    Ok(containers.into_iter().filter_map(|c| c.id).collect())
}

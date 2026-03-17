use std::collections::HashMap;

use bollard::container::{Config, CreateContainerOptions, ListContainersOptions, StartContainerOptions};
use bollard::image::CreateImageOptions;
use bollard::models::{HostConfig, Mount, MountTypeEnum, PortBinding, RestartPolicy, RestartPolicyNameEnum};
use bollard::network::CreateNetworkOptions;
use bollard::Docker;
use futures::StreamExt;

pub const NETWORK_NAME: &str = "pulse-gateway";
pub const CADDY_CONTAINER_NAME: &str = "pulse-caddy";

pub fn connect() -> Result<Docker, String> {
    Docker::connect_with_local_defaults().map_err(|e| format!("Docker connection failed: {e}"))
}

pub async fn ensure_network(docker: &Docker) -> Result<(), String> {
    let networks = docker
        .list_networks::<String>(None)
        .await
        .map_err(|e| format!("Failed to list networks: {e}"))?;

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
            .map_err(|e| format!("Failed to create network: {e}"))?;
    }

    Ok(())
}

pub async fn ensure_caddy(docker: &Docker, image: &str) -> Result<(), String> {
    // Check if container already exists
    let filters: HashMap<String, Vec<String>> = HashMap::from([(
        "name".to_string(),
        vec![CADDY_CONTAINER_NAME.to_string()],
    )]);
    let containers = docker
        .list_containers(Some(ListContainersOptions {
            all: true,
            filters,
            ..Default::default()
        }))
        .await
        .map_err(|e| format!("Failed to list containers: {e}"))?;

    if let Some(container) = containers.first() {
        // Container exists — start it if not running
        let state = container.state.as_deref().unwrap_or("");
        if state != "running" {
            docker
                .start_container(CADDY_CONTAINER_NAME, None::<StartContainerOptions<String>>)
                .await
                .map_err(|e| format!("Failed to start Caddy: {e}"))?;
        }
        return Ok(());
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
        result.map_err(|e| format!("Failed to pull image: {e}"))?;
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
        .map_err(|e| format!("Failed to create Caddy container: {e}"))?;

    docker
        .start_container(CADDY_CONTAINER_NAME, None::<StartContainerOptions<String>>)
        .await
        .map_err(|e| format!("Failed to start Caddy: {e}"))?;

    Ok(())
}

pub async fn is_caddy_running(docker: &Docker) -> bool {
    let filters: HashMap<String, Vec<String>> = HashMap::from([(
        "name".to_string(),
        vec![CADDY_CONTAINER_NAME.to_string()],
    )]);
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

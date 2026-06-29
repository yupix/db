use bollard::Docker;
use bollard::container::{
    Config as ContainerConfig, CreateContainerOptions, RemoveContainerOptions,
    StartContainerOptions, StopContainerOptions,
};
use bollard::models::{HostConfig, PortBinding};
use std::collections::HashMap;

use crate::error::AppError;

pub async fn create_postgres_container(
    docker: &Docker,
    container_name: &str,
    db_name: &str,
    db_user: &str,
    db_password: &str,
    port: u16,
) -> Result<String, AppError> {
    let mut port_bindings = HashMap::new();
    port_bindings.insert(
        "5432/tcp".to_string(),
        Some(vec![PortBinding {
            host_ip: Some("0.0.0.0".to_string()),
            host_port: Some(port.to_string()),
        }]),
    );

    let pg_db = format!("POSTGRES_DB={}", db_name);
    let pg_user = format!("POSTGRES_USER={}", db_user);
    let pg_pass = format!("POSTGRES_PASSWORD={}", db_password);
    let env = vec![pg_db.as_str(), pg_user.as_str(), pg_pass.as_str()];

    let host_config = HostConfig {
        port_bindings: Some(port_bindings),
        ..Default::default()
    };

    let config = ContainerConfig {
        image: Some("postgres:16-alpine"),
        env: Some(env),
        host_config: Some(host_config),
        ..Default::default()
    };

    let options = Some(CreateContainerOptions {
        name: container_name,
        platform: None,
    });

    let result = docker.create_container(options, config).await?;
    docker
        .start_container(&result.id, None::<StartContainerOptions<String>>)
        .await?;

    tracing::info!(
        "Created and started container: {} ({})",
        container_name,
        result.id
    );
    Ok(result.id)
}

pub async fn start_container(docker: &Docker, container_id: &str) -> Result<(), AppError> {
    docker
        .start_container(container_id, None::<StartContainerOptions<String>>)
        .await?;
    Ok(())
}

pub async fn stop_container(docker: &Docker, container_id: &str) -> Result<(), AppError> {
    docker
        .stop_container(container_id, None::<StopContainerOptions>)
        .await?;
    Ok(())
}

pub async fn remove_container(docker: &Docker, container_id: &str) -> Result<(), AppError> {
    let options = Some(RemoveContainerOptions {
        force: true,
        ..Default::default()
    });
    docker.remove_container(container_id, options).await?;
    Ok(())
}
